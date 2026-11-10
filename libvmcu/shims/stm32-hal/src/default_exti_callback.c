/*
 * libvmcu/shims/stm32-hal/src/default_exti_callback.c — HAL_GPIO_EXTI_Callback par défaut.
 *
 * Équivalent de la fonction __weak du HAL de ST. Seule dans son fichier : la
 * bibliothèque statique ne l'utilise que si le projet ne la définit pas, sans
 * dépendre de __attribute__((weak)), mal géré par MinGW (format PE/COFF).
 */
#include "stm32f4xx_hal.h"

void HAL_GPIO_EXTI_Callback(uint16_t GPIO_Pin)
{
    UNUSED(GPIO_Pin);
}
