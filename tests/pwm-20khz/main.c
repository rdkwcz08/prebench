/*
 * tests/pwm-20khz/main.c — PWM à 20 kHz sous forte cadence de consignes.
 *
 * TIM2_CH1 sur PA5 (AF1) : 84 MHz / 4200 = 20 kHz. Pendant 3 s, le rapport
 * cyclique change toutes les 100 µs (rampe de 0 à 100 %, 30 000 consignes),
 * puis il reste à 50 % : LD2 à mi-luminosité. Une PWM qui tourne n'envoie
 * aucun message ; libvmcu regroupe les consignes (au plus une par lot de
 * 1 ms), et la boucle dort entre deux consignes : ni le canal ni le CPU ne
 * saturent.
 */
#include <stdio.h>

#include "stm32f4xx_hal.h"

TIM_HandleTypeDef htim2;

/* Horloge de CubeMX pour la Nucleo-F401RE : PLL à 84 MHz, timers à 84 MHz. */
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
    HAL_RCC_OscConfig(&osc);
    RCC_ClkInitTypeDef clk = {0};
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

    GPIO_InitTypeDef pin = {0};
    pin.Pin = GPIO_PIN_5;
    pin.Mode = GPIO_MODE_AF_PP;
    pin.Alternate = GPIO_AF1_TIM2;
    HAL_GPIO_Init(GPIOA, &pin);

    htim2.Instance = TIM2;
    htim2.Init.Prescaler = 0;
    htim2.Init.CounterMode = TIM_COUNTERMODE_UP;
    htim2.Init.Period = 4199;
    HAL_TIM_PWM_Init(&htim2);
    TIM_OC_InitTypeDef oc = {0};
    oc.OCMode = TIM_OCMODE_PWM1;
    oc.OCPolarity = TIM_OCPOLARITY_HIGH;
    HAL_TIM_PWM_ConfigChannel(&htim2, &oc, TIM_CHANNEL_1);
    HAL_TIM_PWM_Start(&htim2, TIM_CHANNEL_1);

    /* 3 s de consignes toutes les 100 µs (temps du firmware). */
    uint32_t start = HAL_GetTick();
    uint32_t n = 0;
    while (HAL_GetTick() - start < 3000U) {
        __HAL_TIM_SET_COMPARE(&htim2, TIM_CHANNEL_1, (n * 7U) % 4200U);
        n++;
        vmcu_delay_us(100);
    }
    __HAL_TIM_SET_COMPARE(&htim2, TIM_CHANNEL_1, 2100);
    printf("pwm-20khz : %lu consignes en 3 s\n", (unsigned long)n);
    fflush(stdout);
    while (1) {
        HAL_Delay(100);
    }
}
