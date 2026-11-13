/*
 * tests/guard-busy-loop/main.c — Garde-fou CPU (section 11.2).
 *
 * Allume LD2, puis tourne dans une boucle sans délai qui n'appelle jamais la
 * HAL : un cœur à 100 %. Prebench doit mettre la simulation en pause au bout
 * de 5 s (PAUSE, puis suspension du processus) et proposer « Reprendre ».
 */
#include "stm32f4xx_hal.h"

int main(void)
{
    HAL_Init();
    __HAL_RCC_GPIOA_CLK_ENABLE();
    GPIO_InitTypeDef ld2 = {0};
    ld2.Pin = GPIO_PIN_5;
    ld2.Mode = GPIO_MODE_OUTPUT_PP;
    HAL_GPIO_Init(GPIOA, &ld2);
    HAL_GPIO_WritePin(GPIOA, GPIO_PIN_5, GPIO_PIN_SET); /* LD2 : le firmware a démarré */

    volatile uint32_t spins = 0;
    while (1) {
        spins++; /* oubli typique : ni HAL_Delay, ni attente d'interruption */
    }
}
