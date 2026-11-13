/*
 * tests/guard-memory-leak/main.c — Garde-fou mémoire (section 11.2).
 *
 * Fait clignoter LD2 et alloue 4 Mo à chaque bascule (toutes les 50 ms), sans
 * jamais libérer : environ 80 Mo/s. Prebench doit arrêter le firmware au-delà
 * de 256 Mo et expliquer pourquoi.
 */
#include <stdlib.h>
#include <string.h>

#include "stm32f4xx_hal.h"

#define LEAK_BYTES (4u * 1024u * 1024u)

int main(void)
{
    HAL_Init();
    __HAL_RCC_GPIOA_CLK_ENABLE();
    GPIO_InitTypeDef ld2 = {0};
    ld2.Pin = GPIO_PIN_5;
    ld2.Mode = GPIO_MODE_OUTPUT_PP;
    HAL_GPIO_Init(GPIOA, &ld2);

    while (1) {
        HAL_GPIO_TogglePin(GPIOA, GPIO_PIN_5);
        char *block = malloc(LEAK_BYTES); /* jamais libéré */
        if (block == NULL) {
            while (1) {
                HAL_Delay(1000); /* allocation refusée par le système : on attend */
            }
        }
        memset(block, 0xA5, LEAK_BYTES); /* pages réellement utilisées */
        HAL_Delay(50);
    }
}
