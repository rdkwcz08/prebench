/*
 * tests/net-checks/main.c — Validations du réseau logique (section 9.2).
 *
 * - A0 (PA0) en entrée sans résistance de tirage, non câblée : Prebench signale
 *   une entrée lue flottante.
 * - PC13 (B1) en sortie push-pull au niveau haut : un appui sur B1 relie la
 *   broche à la masse ; deux sorties s'opposent, Prebench signale un conflit.
 */
#include "stm32f4xx_hal.h"

int main(void)
{
    HAL_Init();
    __HAL_RCC_GPIOA_CLK_ENABLE();
    __HAL_RCC_GPIOC_CLK_ENABLE();

    GPIO_InitTypeDef in = {0};
    in.Pin = GPIO_PIN_0;
    in.Mode = GPIO_MODE_INPUT;
    in.Pull = GPIO_NOPULL; /* oubli typique : ni pull-up ni pull-down */
    HAL_GPIO_Init(GPIOA, &in);

    GPIO_InitTypeDef out = {0};
    out.Pin = GPIO_PIN_13;
    out.Mode = GPIO_MODE_OUTPUT_PP; /* erreur volontaire : B1 est câblé à la masse */
    HAL_GPIO_Init(GPIOC, &out);
    HAL_GPIO_WritePin(GPIOC, GPIO_PIN_13, GPIO_PIN_SET);

    while (1) {
        (void)HAL_GPIO_ReadPin(GPIOA, GPIO_PIN_0);
        HAL_Delay(100);
    }
}
