/*
 * tests/ws2812-chase/main.c — Ruban WS2812 par TIM1 et DMA (phase 3, étape 8).
 *
 * Écrit comme les projets STM32CubeMX des tutoriels : TIM1 canal 1 en PWM à
 * 800 kHz (84 MHz / 105), PA8 (D7) en AF1, DMA2 Stream1 lié au canal 1 en
 * demi-mots (__HAL_LINKDMA). Chaque bit du ruban est une valeur de comparaison
 * dans le tampon : 67 (≈ 0,8 µs) pour 1, 34 (≈ 0,4 µs) pour 0, puis 50 zéros
 * de repos (reset). HAL_TIM_PWM_PulseFinishedCallback arrête le DMA et lève un
 * drapeau ; le même firmware tourne sur la vraie carte.
 * Animation : un point rouge court le long des 8 LED avec une traînée
 * dégradée (2 tours), puis le ruban s'allume en arc-en-ciel qui tourne
 * (2 tours) ; une image toutes les 50 ms. USART2 : « image:<n> » pour le
 * traceur.
 */
#include <stdio.h>
#include <string.h>

#include "stm32f4xx_hal.h"

#define LEDS        8u
#define T1H         67u
#define T0H         34u
#define RESET_SLOTS 50u

UART_HandleTypeDef huart2;
TIM_HandleTypeDef htim1;
DMA_HandleTypeDef hdma_tim1_ch1;

/* Déclarée dans le main.h d'un projet CubeMX. */
void HAL_TIM_MspPostInit(TIM_HandleTypeDef *htim);

static uint16_t pwm[LEDS * 24u + RESET_SLOTS];
static uint8_t grb[LEDS][3];
static volatile int sent;

#define PRINT(...)                                                                  \
    do {                                                                            \
        char line_[48];                                                             \
        int len_ = snprintf(line_, sizeof line_, __VA_ARGS__);                      \
        HAL_UART_Transmit(&huart2, (const uint8_t *)line_, (uint16_t)len_, 100);    \
    } while (0)

static void set_led(unsigned i, uint8_t r, uint8_t g, uint8_t b)
{
    grb[i][0] = g;
    grb[i][1] = r;
    grb[i][2] = b;
}

/* Tampon DMA de la trame, puis envoi ; attend la fin (callback). */
static void show(void)
{
    unsigned k = 0;
    for (unsigned i = 0; i < LEDS; ++i) {
        for (unsigned c = 0; c < 3; ++c) {
            for (int bit = 7; bit >= 0; --bit) {
                pwm[k++] = ((grb[i][c] >> bit) & 1u) ? T1H : T0H;
            }
        }
    }
    while (k < sizeof pwm / sizeof pwm[0]) {
        pwm[k++] = 0;
    }
    sent = 0;
    HAL_TIM_PWM_Start_DMA(&htim1, TIM_CHANNEL_1, (uint32_t *)pwm, (uint16_t)(sizeof pwm / sizeof pwm[0]));
    while (!sent) {
        HAL_Delay(1);
    }
}

void HAL_TIM_PWM_PulseFinishedCallback(TIM_HandleTypeDef *htim)
{
    if (htim == &htim1) {
        HAL_TIM_PWM_Stop_DMA(&htim1, TIM_CHANNEL_1);
        sent = 1;
    }
}

/* Teinte 0..255 -> couleur saturée, luminosité réduite (ruban alimenté par USB). */
static void wheel(uint8_t pos, uint8_t *r, uint8_t *g, uint8_t *b)
{
    pos = (uint8_t)(255u - pos);
    if (pos < 85u) {
        *r = (uint8_t)((255u - pos * 3u) / 4u); *g = 0; *b = (uint8_t)(pos * 3u / 4u);
    } else if (pos < 170u) {
        pos = (uint8_t)(pos - 85u);
        *r = 0; *g = (uint8_t)(pos * 3u / 4u); *b = (uint8_t)((255u - pos * 3u) / 4u);
    } else {
        pos = (uint8_t)(pos - 170u);
        *r = (uint8_t)(pos * 3u / 4u); *g = (uint8_t)((255u - pos * 3u) / 4u); *b = 0;
    }
}

/* --- CubeMX ------------------------------------------------------------------------------ */

void HAL_TIM_MspPostInit(TIM_HandleTypeDef *htim)
{
    if (htim->Instance == TIM1) {
        GPIO_InitTypeDef gpio = { 0 };
        __HAL_RCC_GPIOA_CLK_ENABLE();
        gpio.Pin = GPIO_PIN_8;
        gpio.Mode = GPIO_MODE_AF_PP;
        gpio.Pull = GPIO_NOPULL;
        gpio.Speed = GPIO_SPEED_FREQ_LOW;
        gpio.Alternate = GPIO_AF1_TIM1;
        HAL_GPIO_Init(GPIOA, &gpio);
    }
}

void HAL_TIM_PWM_MspInit(TIM_HandleTypeDef *htim)
{
    if (htim->Instance == TIM1) {
        __HAL_RCC_TIM1_CLK_ENABLE();
        __HAL_RCC_DMA2_CLK_ENABLE();
        hdma_tim1_ch1.Instance = DMA2_Stream1;
        hdma_tim1_ch1.Init.Channel = DMA_CHANNEL_6;
        hdma_tim1_ch1.Init.Direction = DMA_MEMORY_TO_PERIPH;
        hdma_tim1_ch1.Init.PeriphInc = DMA_PINC_DISABLE;
        hdma_tim1_ch1.Init.MemInc = DMA_MINC_ENABLE;
        hdma_tim1_ch1.Init.PeriphDataAlignment = DMA_PDATAALIGN_HALFWORD;
        hdma_tim1_ch1.Init.MemDataAlignment = DMA_MDATAALIGN_HALFWORD;
        hdma_tim1_ch1.Init.Mode = DMA_NORMAL;
        hdma_tim1_ch1.Init.Priority = DMA_PRIORITY_LOW;
        HAL_DMA_Init(&hdma_tim1_ch1);
        __HAL_LINKDMA(htim, hdma[TIM_DMA_ID_CC1], hdma_tim1_ch1);
        HAL_NVIC_SetPriority(DMA2_Stream1_IRQn, 0, 0);
        HAL_NVIC_EnableIRQ(DMA2_Stream1_IRQn);
    }
}

static void MX_TIM1_Init(void)
{
    htim1.Instance = TIM1;
    htim1.Init.Prescaler = 0;
    htim1.Init.CounterMode = TIM_COUNTERMODE_UP;
    htim1.Init.Period = 104;
    htim1.Init.ClockDivision = TIM_CLOCKDIVISION_DIV1;
    HAL_TIM_PWM_Init(&htim1);
    TIM_OC_InitTypeDef oc = { 0 };
    oc.OCMode = TIM_OCMODE_PWM1;
    oc.Pulse = 0;
    oc.OCPolarity = TIM_OCPOLARITY_HIGH;
    HAL_TIM_PWM_ConfigChannel(&htim1, &oc, TIM_CHANNEL_1);
    HAL_TIM_MspPostInit(&htim1);
}

static void MX_USART2_UART_Init(void)
{
    __HAL_RCC_USART2_CLK_ENABLE();
    huart2.Instance = USART2;
    huart2.Init.BaudRate = 115200;
    huart2.Init.WordLength = UART_WORDLENGTH_8B;
    huart2.Init.StopBits = UART_STOPBITS_1;
    huart2.Init.Parity = UART_PARITY_NONE;
    huart2.Init.Mode = UART_MODE_TX_RX;
    HAL_UART_Init(&huart2);
}

static void SystemClock_Config(void)
{
    RCC_OscInitTypeDef osc = { 0 };
    RCC_ClkInitTypeDef clk = { 0 };
    osc.OscillatorType = RCC_OSCILLATORTYPE_HSI;
    osc.HSIState = RCC_HSI_ON;
    osc.PLL.PLLState = RCC_PLL_ON;
    osc.PLL.PLLSource = RCC_PLLSOURCE_HSI;
    osc.PLL.PLLM = 16;
    osc.PLL.PLLN = 336;
    osc.PLL.PLLP = RCC_PLLP_DIV4;
    osc.PLL.PLLQ = 7;
    HAL_RCC_OscConfig(&osc);
    clk.ClockType = RCC_CLOCKTYPE_HCLK | RCC_CLOCKTYPE_SYSCLK | RCC_CLOCKTYPE_PCLK1 | RCC_CLOCKTYPE_PCLK2;
    clk.SYSCLKSource = RCC_SYSCLKSOURCE_PLLCLK;
    clk.AHBCLKDivider = RCC_SYSCLK_DIV1;
    clk.APB1CLKDivider = RCC_HCLK_DIV2;
    clk.APB2CLKDivider = RCC_HCLK_DIV1;
    HAL_RCC_ClockConfig(&clk, FLASH_LATENCY_2);
}

int main(void)
{
    HAL_Init();
    SystemClock_Config();
    MX_USART2_UART_Init();
    MX_TIM1_Init();

    unsigned frames = 0;
    for (;;) {
        /* Point rouge à traînée, deux tours. */
        for (unsigned step = 0; step < 2u * LEDS; ++step) {
            for (unsigned i = 0; i < LEDS; ++i) {
                unsigned d = (step + LEDS - i) % LEDS; /* distance derrière la tête */
                uint8_t r = d == 0 ? 64u : d == 1 ? 24u : d == 2 ? 8u : 0u;
                set_led(i, r, 0, 0);
            }
            show();
            PRINT("image:%u\r\n", ++frames);
            HAL_Delay(50);
        }
        /* Arc-en-ciel qui tourne, deux tours. */
        for (unsigned step = 0; step < 64u; ++step) {
            for (unsigned i = 0; i < LEDS; ++i) {
                uint8_t r, g, b;
                wheel((uint8_t)(i * 256u / LEDS + step * 8u), &r, &g, &b);
                set_led(i, r, g, b);
            }
            show();
            PRINT("image:%u\r\n", ++frames);
            HAL_Delay(50);
        }
    }
}
