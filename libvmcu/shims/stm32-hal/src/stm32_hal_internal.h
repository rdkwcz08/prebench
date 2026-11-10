/*
 * libvmcu/shims/stm32-hal/src/stm32_hal_internal.h — Partagé entre les fichiers du shim.
 */
#ifndef PREBENCH_STM32_HAL_INTERNAL_H
#define PREBENCH_STM32_HAL_INTERNAL_H

#include "stm32f4xx_hal.h"

/** true si l'IRQ est activée dans le NVIC (HAL_NVIC_EnableIRQ). */
bool prebench_nvic_enabled(IRQn_Type irqn);

/** IRQ EXTI qui dessert une ligne (0..15). */
IRQn_Type prebench_exti_irqn(uint32_t line);

/** Appelle le gestionnaire EXTI (projet ou shim) correspondant à l'IRQ. */
void prebench_exti_call_handler(IRQn_Type irqn);

/** Lignes EXTI en attente servies par une IRQ (masque de bits). */
uint32_t prebench_exti_lines_of(IRQn_Type irqn);

/** Avertit une fois « <fonction> non simulé ». */
#define PREBENCH_UNSIMULATED() vmcu_warn_unsimulated(__func__)

/** Réception non simulée : attend au plus 10 ms (jamais d'attente active), puis expire. */
void prebench_stub_wait(uint32_t timeout_ms);

#endif /* PREBENCH_STM32_HAL_INTERNAL_H */
