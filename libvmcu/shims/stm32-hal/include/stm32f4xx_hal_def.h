/*
 * libvmcu/shims/stm32-hal/include/stm32f4xx_hal_def.h — Définitions communes du HAL (shim).
 */
#ifndef PREBENCH_STM32F4XX_HAL_DEF_H
#define PREBENCH_STM32F4XX_HAL_DEF_H

#include "stm32f4xx.h"

#ifdef __cplusplus
extern "C" {
#endif

typedef enum {
    HAL_OK      = 0x00U,
    HAL_ERROR   = 0x01U,
    HAL_BUSY    = 0x02U,
    HAL_TIMEOUT = 0x03U
} HAL_StatusTypeDef;

typedef enum {
    HAL_UNLOCKED = 0x00U,
    HAL_LOCKED   = 0x01U
} HAL_LockTypeDef;

typedef enum { RESET = 0U, SET = !RESET } FlagStatus, ITStatus;
typedef enum { DISABLE = 0U, ENABLE = !DISABLE } FunctionalState;
typedef enum { SUCCESS = 0U, ERROR = !SUCCESS } ErrorStatus;

#define HAL_MAX_DELAY 0xFFFFFFFFU
#define UNUSED(X)     (void)(X)
#ifndef assert_param
#define assert_param(expr) ((void)0U)
#endif

/* Associe un handle DMA à un handle de périphérique (msp.c de CubeMX). */
#define __HAL_LINKDMA(__HANDLE__, __PPP_DMA_FIELD__, __DMA_HANDLE__) \
    do {                                                             \
        (__HANDLE__)->__PPP_DMA_FIELD__ = &(__DMA_HANDLE__);         \
        (__DMA_HANDLE__).Parent = (__HANDLE__);                      \
    } while (0U)

#define __HAL_LOCK(__HANDLE__)   do { (__HANDLE__)->Lock = HAL_LOCKED; } while (0U)
#define __HAL_UNLOCK(__HANDLE__) do { (__HANDLE__)->Lock = HAL_UNLOCKED; } while (0U)

#ifdef __cplusplus
}
#endif

#endif /* PREBENCH_STM32F4XX_HAL_DEF_H */
