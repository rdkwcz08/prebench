/*
 * tests/budget/main.c — Paillasse de mesure des budgets (cahier des charges, section 11.1).
 *
 * Tous les composants sont actifs en permanence, comme dans un vrai projet :
 * - TIM2 à 440 Hz : CH1 (PA5, LD2) en fondu, CH3 (PB10, D6) pour le buzzer,
 *   une demi-seconde sur deux ;
 * - TIM3 à 50 Hz : CH1 (PB4, D5) pour le servo, CH2 (PC7, D9) pour le rouge ;
 * - TIM1 à 1 kHz : CH1 (PA8, D7) pour le vert, en fondu inverse ;
 * - PB5 (D4) : bleu, allumé tant que le bouton sur PA10 (D2) est appuyé ;
 * - ADC1 IN0 (PA0, A0) : potentiomètre -> servo et rouge, toutes les 10 ms ;
 * - USART2 : « pot:<valeur>,ld2:<pour mille> » toutes les 100 ms (traceur) ;
 * - I2C1 (PB8 SCL, PB9 SDA) : écran OLED SSD1306 en 0x3C, réécrit en entier
 *   toutes les 100 ms (1 024 octets, phase 3) : barre du potentiomètre et
 *   trait vertical qui défile.
 */
#include <stdio.h>

#include "stm32f4xx_hal.h"

static TIM_HandleTypeDef htim1, htim2, htim3;
static ADC_HandleTypeDef hadc1;
static UART_HandleTypeDef huart2;
static I2C_HandleTypeDef hi2c1;
static uint8_t oled[128 * 8];

static void oled_cmds(const uint8_t *cmds, uint16_t n)
{
    HAL_I2C_Mem_Write(&hi2c1, 0x3C << 1, 0x00, I2C_MEMADD_SIZE_8BIT, (uint8_t *)cmds, n, 10);
}

/* Image entière : barre (pages 2 et 3) selon le potentiomètre, trait vertical en x. */
static void oled_frame(uint32_t raw, uint32_t x)
{
    static const uint8_t window[] = { 0x21, 0, 127, 0x22, 0, 7 };
    for (uint32_t c = 0; c < 128; ++c) {
        uint8_t bar = c < raw * 128U / 4096U ? 0xFF : 0x00;
        oled[0 * 128 + c] = (c % 8U == 0U) ? 0x0F : 0x00;
        oled[2 * 128 + c] = bar;
        oled[3 * 128 + c] = bar;
        for (uint32_t p = 4; p < 8; ++p) {
            oled[p * 128 + c] = (c == x) ? 0xFF : 0x00;
        }
    }
    oled_cmds(window, sizeof window);
    HAL_I2C_Mem_Write(&hi2c1, 0x3C << 1, 0x40, I2C_MEMADD_SIZE_8BIT, oled, sizeof oled, 10);
}

static void SystemClock_Config(void)
{
    RCC_OscInitTypeDef osc = {0};
    osc.OscillatorType = RCC_OSCILLATORTYPE_HSI;
    osc.HSIState = RCC_HSI_ON;
    osc.PLL.PLLState = RCC_PLL_ON;
    osc.PLL.PLLSource = RCC_PLLSOURCE_HSI;
    osc.PLL.PLLM = 16;
    osc.PLL.PLLN = 336;
    osc.PLL.PLLP = RCC_PLLP_DIV4;
    osc.PLL.PLLQ = 7;
    HAL_RCC_OscConfig(&osc);
    RCC_ClkInitTypeDef clk = {0};
    clk.ClockType = RCC_CLOCKTYPE_HCLK | RCC_CLOCKTYPE_SYSCLK | RCC_CLOCKTYPE_PCLK1 | RCC_CLOCKTYPE_PCLK2;
    clk.SYSCLKSource = RCC_SYSCLKSOURCE_PLLCLK;
    clk.AHBCLKDivider = RCC_SYSCLK_DIV1;
    clk.APB1CLKDivider = RCC_HCLK_DIV2;
    clk.APB2CLKDivider = RCC_HCLK_DIV1;
    HAL_RCC_ClockConfig(&clk, FLASH_LATENCY_2);
}

static void pin(GPIO_TypeDef *port, uint16_t pin, uint32_t mode, uint32_t pull, uint8_t af)
{
    GPIO_InitTypeDef g = {0};
    g.Pin = pin;
    g.Mode = mode;
    g.Pull = pull;
    g.Alternate = af;
    HAL_GPIO_Init(port, &g);
}

/* Timer en PWM : prescaler 83 (1 MHz), période donnée, canaux à 0. */
static void pwm_timer(TIM_HandleTypeDef *h, TIM_TypeDef *tim, uint32_t period, const uint32_t *channels, int n)
{
    h->Instance = tim;
    h->Init.Prescaler = 83;
    h->Init.CounterMode = TIM_COUNTERMODE_UP;
    h->Init.Period = period;
    HAL_TIM_PWM_Init(h);
    TIM_OC_InitTypeDef oc = {0};
    oc.OCMode = TIM_OCMODE_PWM1;
    oc.OCPolarity = TIM_OCPOLARITY_HIGH;
    for (int i = 0; i < n; ++i) {
        HAL_TIM_PWM_ConfigChannel(h, &oc, channels[i]);
        HAL_TIM_PWM_Start(h, channels[i]);
    }
}

int main(void)
{
    HAL_Init();
    SystemClock_Config();

    pin(GPIOA, GPIO_PIN_5, GPIO_MODE_AF_PP, GPIO_NOPULL, GPIO_AF1_TIM2);   /* LD2 */
    pin(GPIOB, GPIO_PIN_10, GPIO_MODE_AF_PP, GPIO_NOPULL, GPIO_AF1_TIM2);  /* buzzer */
    pin(GPIOB, GPIO_PIN_4, GPIO_MODE_AF_PP, GPIO_NOPULL, GPIO_AF2_TIM3);   /* servo */
    pin(GPIOC, GPIO_PIN_7, GPIO_MODE_AF_PP, GPIO_NOPULL, GPIO_AF2_TIM3);   /* rouge */
    pin(GPIOA, GPIO_PIN_8, GPIO_MODE_AF_PP, GPIO_NOPULL, GPIO_AF1_TIM1);   /* vert */
    pin(GPIOB, GPIO_PIN_5, GPIO_MODE_OUTPUT_PP, GPIO_NOPULL, 0);           /* bleu */
    pin(GPIOA, GPIO_PIN_10, GPIO_MODE_INPUT, GPIO_PULLUP, 0);              /* bouton */
    pin(GPIOA, GPIO_PIN_0, GPIO_MODE_ANALOG, GPIO_NOPULL, 0);              /* potentiomètre */
    pin(GPIOB, GPIO_PIN_8 | GPIO_PIN_9, GPIO_MODE_AF_OD, GPIO_NOPULL, GPIO_AF4_I2C1); /* OLED */

    const uint32_t ch2[] = { TIM_CHANNEL_1, TIM_CHANNEL_3 };
    const uint32_t ch3[] = { TIM_CHANNEL_1, TIM_CHANNEL_2 };
    const uint32_t ch1[] = { TIM_CHANNEL_1 };
    pwm_timer(&htim2, TIM2, 2272, ch2, 2);   /* 440 Hz */
    pwm_timer(&htim3, TIM3, 19999, ch3, 2);  /* 50 Hz */
    pwm_timer(&htim1, TIM1, 999, ch1, 1);    /* 1 kHz */

    hadc1.Instance = ADC1;
    hadc1.Init.Resolution = ADC_RESOLUTION_12B;
    hadc1.Init.NbrOfConversion = 1;
    HAL_ADC_Init(&hadc1);
    ADC_ChannelConfTypeDef sc = {0};
    sc.Channel = ADC_CHANNEL_0;
    sc.Rank = 1;
    HAL_ADC_ConfigChannel(&hadc1, &sc);

    huart2.Instance = USART2;
    huart2.Init.BaudRate = 115200;
    huart2.Init.Mode = UART_MODE_TX_RX;
    HAL_UART_Init(&huart2);

    hi2c1.Instance = I2C1;
    hi2c1.Init.ClockSpeed = 400000;
    hi2c1.Init.AddressingMode = I2C_ADDRESSINGMODE_7BIT;
    HAL_I2C_Init(&hi2c1);
    static const uint8_t oled_init[] = {
        0xAE, 0xD5, 0x80, 0xA8, 0x3F, 0xD3, 0x00, 0x40, 0x8D, 0x14, 0x20, 0x00,
        0xA1, 0xC8, 0xDA, 0x12, 0x81, 0xCF, 0xD9, 0xF1, 0xDB, 0x40, 0xA4, 0xA6, 0xAF,
    };
    oled_cmds(oled_init, sizeof oled_init);

    uint32_t tick = 0;
    int32_t fade = 0, step = 23;
    while (1) {
        HAL_ADC_Start(&hadc1);
        HAL_ADC_PollForConversion(&hadc1, 10);
        uint32_t raw = HAL_ADC_GetValue(&hadc1);

        __HAL_TIM_SET_COMPARE(&htim3, TIM_CHANNEL_1, 1000U + raw * 1000U / 4095U); /* servo */
        __HAL_TIM_SET_COMPARE(&htim3, TIM_CHANNEL_2, raw * 20000U / 4095U);        /* rouge */
        fade += step;
        if (fade >= 2273 || fade <= 0) {
            step = -step;
            fade = fade < 0 ? 0 : 2273;
        }
        __HAL_TIM_SET_COMPARE(&htim2, TIM_CHANNEL_1, (uint32_t)fade);                 /* LD2 */
        __HAL_TIM_SET_COMPARE(&htim1, TIM_CHANNEL_1, 1000U - (uint32_t)fade * 1000U / 2273U); /* vert */
        __HAL_TIM_SET_COMPARE(&htim2, TIM_CHANNEL_3, (tick / 50U) % 2U == 0U ? 1136U : 0U); /* buzzer */
        HAL_GPIO_WritePin(GPIOB, GPIO_PIN_5,
                          HAL_GPIO_ReadPin(GPIOA, GPIO_PIN_10) == GPIO_PIN_RESET ? GPIO_PIN_SET : GPIO_PIN_RESET);

        if (tick % 10U == 0U) {
            char line[40];
            int len = snprintf(line, sizeof line, "pot:%lu,ld2:%lu\r\n", (unsigned long)raw,
                               (unsigned long)fade * 1000UL / 2273UL);
            HAL_UART_Transmit(&huart2, (uint8_t *)line, (uint16_t)len, 10);
            oled_frame(raw, (tick / 10U) % 128U);
        }
        tick++;
        HAL_Delay(10);
    }
}
