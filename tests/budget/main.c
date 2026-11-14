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
 * - USART2 : « pot:<valeur>,ld2:<pour mille> » toutes les 100 ms (traceur).
 */
#include <stdio.h>

#include "stm32f4xx_hal.h"

static TIM_HandleTypeDef htim1, htim2, htim3;
static ADC_HandleTypeDef hadc1;
static UART_HandleTypeDef huart2;

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
        }
        tick++;
        HAL_Delay(10);
    }
}
