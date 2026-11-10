/*
 * libvmcu/shims/stm32-hal/include/stm32f4xx_hal.h — Shim STM32 HAL (cible « sim »).
 *
 * Expose les mêmes noms que le HAL de ST pour qu'un projet CubeMX compile sans
 * modification. Chaque fonction traduit l'appel vers l'API vmcu_* (règle 16.2.2).
 *
 * Simulés en phase 1 : HAL de base (Init, tick, délais), RCC, PWR, Cortex/NVIC,
 * GPIO et EXTI. Modules DMA, UART, TIM, ADC, I2C et SPI : types et fonctions
 * complets, qui compilent et avertissent « non simulé » (règle 16.2.10) ; leur
 * simulation arrive en phase 2 (UART, PWM, ADC) et 3 (I2C, SPI).
 *
 * Interruptions : un front sur une broche configurée en GPIO_MODE_IT_* appelle,
 * si l'IRQ EXTI est activée dans le NVIC, le gestionnaire EXTIx_IRQHandler du
 * projet (stm32f4xx_it.c), sinon celui du shim ; puis HAL_GPIO_EXTI_IRQHandler
 * appelle HAL_GPIO_EXTI_Callback. Tout cela s'exécute dans le fil IRQ de
 * libvmcu, verrou d'interruption tenu.
 */
#ifndef PREBENCH_STM32F4XX_HAL_H
#define PREBENCH_STM32F4XX_HAL_H

#include "stm32f4xx_hal_def.h"

#ifdef __cplusplus
extern "C" {
#endif

/* --- HAL de base ---------------------------------------------------------------------------- */

typedef enum {
    HAL_TICK_FREQ_10HZ    = 100U,
    HAL_TICK_FREQ_100HZ   = 10U,
    HAL_TICK_FREQ_1KHZ    = 1U,
    HAL_TICK_FREQ_DEFAULT = HAL_TICK_FREQ_1KHZ
} HAL_TickFreqTypeDef;

#define TICK_INT_PRIORITY 0x0FU

HAL_StatusTypeDef HAL_Init(void);
HAL_StatusTypeDef HAL_DeInit(void);
void HAL_MspInit(void);
void HAL_MspDeInit(void);
HAL_StatusTypeDef HAL_InitTick(uint32_t TickPriority);
void HAL_IncTick(void);
void HAL_Delay(uint32_t Delay);
uint32_t HAL_GetTick(void);
uint32_t HAL_GetTickPrio(void);
HAL_StatusTypeDef HAL_SetTickFreq(HAL_TickFreqTypeDef Freq);
HAL_TickFreqTypeDef HAL_GetTickFreq(void);
void HAL_SuspendTick(void);
void HAL_ResumeTick(void);
uint32_t HAL_GetHalVersion(void);
uint32_t HAL_GetREVID(void);
uint32_t HAL_GetDEVID(void);

/* --- RCC ------------------------------------------------------------------------------------ */

typedef struct {
    uint32_t PLLState;
    uint32_t PLLSource;
    uint32_t PLLM;
    uint32_t PLLN;
    uint32_t PLLP;
    uint32_t PLLQ;
} RCC_PLLInitTypeDef;

typedef struct {
    uint32_t OscillatorType;
    uint32_t HSEState;
    uint32_t LSEState;
    uint32_t HSIState;
    uint32_t HSICalibrationValue;
    uint32_t LSIState;
    RCC_PLLInitTypeDef PLL;
} RCC_OscInitTypeDef;

typedef struct {
    uint32_t ClockType;
    uint32_t SYSCLKSource;
    uint32_t AHBCLKDivider;
    uint32_t APB1CLKDivider;
    uint32_t APB2CLKDivider;
} RCC_ClkInitTypeDef;

#define RCC_OSCILLATORTYPE_NONE  0x00000000U
#define RCC_OSCILLATORTYPE_HSE   0x00000001U
#define RCC_OSCILLATORTYPE_HSI   0x00000002U
#define RCC_OSCILLATORTYPE_LSE   0x00000004U
#define RCC_OSCILLATORTYPE_LSI   0x00000008U
#define RCC_HSE_OFF              0x00000000U
#define RCC_HSE_ON               0x00010000U
#define RCC_HSE_BYPASS           0x00050000U
#define RCC_LSE_OFF              0x00000000U
#define RCC_LSE_ON               0x00000001U
#define RCC_LSE_BYPASS           0x00000005U
#define RCC_HSI_OFF              0x00000000U
#define RCC_HSI_ON               0x00000001U
#define RCC_HSICALIBRATION_DEFAULT 0x10U
#define RCC_LSI_OFF              0x00000000U
#define RCC_LSI_ON               0x00000001U
#define RCC_PLL_NONE             0x00000000U
#define RCC_PLL_OFF              0x00000001U
#define RCC_PLL_ON               0x00000002U
#define RCC_PLLSOURCE_HSI        0x00000000U
#define RCC_PLLSOURCE_HSE        0x00400000U
#define RCC_PLLP_DIV2            0x00000002U
#define RCC_PLLP_DIV4            0x00000004U
#define RCC_PLLP_DIV6            0x00000006U
#define RCC_PLLP_DIV8            0x00000008U
#define RCC_CLOCKTYPE_SYSCLK     0x00000001U
#define RCC_CLOCKTYPE_HCLK       0x00000002U
#define RCC_CLOCKTYPE_PCLK1      0x00000004U
#define RCC_CLOCKTYPE_PCLK2      0x00000008U
#define RCC_SYSCLKSOURCE_HSI     0x00000000U
#define RCC_SYSCLKSOURCE_HSE     0x00000001U
#define RCC_SYSCLKSOURCE_PLLCLK  0x00000002U
#define RCC_SYSCLK_DIV1          0x00000000U
#define RCC_SYSCLK_DIV2          0x00000080U
#define RCC_SYSCLK_DIV4          0x00000090U
#define RCC_SYSCLK_DIV8          0x000000A0U
#define RCC_SYSCLK_DIV16         0x000000B0U
#define RCC_SYSCLK_DIV64         0x000000C0U
#define RCC_SYSCLK_DIV128        0x000000D0U
#define RCC_SYSCLK_DIV256        0x000000E0U
#define RCC_SYSCLK_DIV512        0x000000F0U
#define RCC_HCLK_DIV1            0x00000000U
#define RCC_HCLK_DIV2            0x00001000U
#define RCC_HCLK_DIV4            0x00001400U
#define RCC_HCLK_DIV8            0x00001800U
#define RCC_HCLK_DIV16           0x00001C00U

#define FLASH_LATENCY_0 0x00000000U
#define FLASH_LATENCY_1 0x00000001U
#define FLASH_LATENCY_2 0x00000002U
#define FLASH_LATENCY_3 0x00000003U
#define FLASH_LATENCY_4 0x00000004U
#define FLASH_LATENCY_5 0x00000005U

HAL_StatusTypeDef HAL_RCC_OscConfig(RCC_OscInitTypeDef *RCC_OscInitStruct);
HAL_StatusTypeDef HAL_RCC_ClockConfig(RCC_ClkInitTypeDef *RCC_ClkInitStruct, uint32_t FLatency);
uint32_t HAL_RCC_GetSysClockFreq(void);
uint32_t HAL_RCC_GetHCLKFreq(void);
uint32_t HAL_RCC_GetPCLK1Freq(void);
uint32_t HAL_RCC_GetPCLK2Freq(void);

/* Horloges des périphériques : aucun effet en simulation. */
#define PREBENCH_NO_EFFECT() do { } while (0)
#define __HAL_RCC_GPIOA_CLK_ENABLE()   PREBENCH_NO_EFFECT()
#define __HAL_RCC_GPIOB_CLK_ENABLE()   PREBENCH_NO_EFFECT()
#define __HAL_RCC_GPIOC_CLK_ENABLE()   PREBENCH_NO_EFFECT()
#define __HAL_RCC_GPIOD_CLK_ENABLE()   PREBENCH_NO_EFFECT()
#define __HAL_RCC_GPIOE_CLK_ENABLE()   PREBENCH_NO_EFFECT()
#define __HAL_RCC_GPIOH_CLK_ENABLE()   PREBENCH_NO_EFFECT()
#define __HAL_RCC_GPIOA_CLK_DISABLE()  PREBENCH_NO_EFFECT()
#define __HAL_RCC_GPIOB_CLK_DISABLE()  PREBENCH_NO_EFFECT()
#define __HAL_RCC_GPIOC_CLK_DISABLE()  PREBENCH_NO_EFFECT()
#define __HAL_RCC_GPIOD_CLK_DISABLE()  PREBENCH_NO_EFFECT()
#define __HAL_RCC_GPIOE_CLK_DISABLE()  PREBENCH_NO_EFFECT()
#define __HAL_RCC_GPIOH_CLK_DISABLE()  PREBENCH_NO_EFFECT()
#define __HAL_RCC_SYSCFG_CLK_ENABLE()  PREBENCH_NO_EFFECT()
#define __HAL_RCC_SYSCFG_CLK_DISABLE() PREBENCH_NO_EFFECT()
#define __HAL_RCC_PWR_CLK_ENABLE()     PREBENCH_NO_EFFECT()
#define __HAL_RCC_PWR_CLK_DISABLE()    PREBENCH_NO_EFFECT()
#define __HAL_RCC_USART1_CLK_ENABLE()  PREBENCH_NO_EFFECT()
#define __HAL_RCC_USART1_CLK_DISABLE() PREBENCH_NO_EFFECT()
#define __HAL_RCC_USART2_CLK_ENABLE()  PREBENCH_NO_EFFECT()
#define __HAL_RCC_USART2_CLK_DISABLE() PREBENCH_NO_EFFECT()
#define __HAL_RCC_USART6_CLK_ENABLE()  PREBENCH_NO_EFFECT()
#define __HAL_RCC_USART6_CLK_DISABLE() PREBENCH_NO_EFFECT()
#define __HAL_RCC_TIM1_CLK_ENABLE()  PREBENCH_NO_EFFECT()
#define __HAL_RCC_TIM1_CLK_DISABLE() PREBENCH_NO_EFFECT()
#define __HAL_RCC_TIM2_CLK_ENABLE()  PREBENCH_NO_EFFECT()
#define __HAL_RCC_TIM2_CLK_DISABLE() PREBENCH_NO_EFFECT()
#define __HAL_RCC_TIM3_CLK_ENABLE()  PREBENCH_NO_EFFECT()
#define __HAL_RCC_TIM3_CLK_DISABLE() PREBENCH_NO_EFFECT()
#define __HAL_RCC_TIM4_CLK_ENABLE()  PREBENCH_NO_EFFECT()
#define __HAL_RCC_TIM4_CLK_DISABLE() PREBENCH_NO_EFFECT()
#define __HAL_RCC_TIM5_CLK_ENABLE()  PREBENCH_NO_EFFECT()
#define __HAL_RCC_TIM5_CLK_DISABLE() PREBENCH_NO_EFFECT()
#define __HAL_RCC_TIM9_CLK_ENABLE()  PREBENCH_NO_EFFECT()
#define __HAL_RCC_TIM9_CLK_DISABLE() PREBENCH_NO_EFFECT()
#define __HAL_RCC_TIM10_CLK_ENABLE()  PREBENCH_NO_EFFECT()
#define __HAL_RCC_TIM10_CLK_DISABLE() PREBENCH_NO_EFFECT()
#define __HAL_RCC_TIM11_CLK_ENABLE()  PREBENCH_NO_EFFECT()
#define __HAL_RCC_TIM11_CLK_DISABLE() PREBENCH_NO_EFFECT()
#define __HAL_RCC_ADC1_CLK_ENABLE()  PREBENCH_NO_EFFECT()
#define __HAL_RCC_ADC1_CLK_DISABLE() PREBENCH_NO_EFFECT()
#define __HAL_RCC_I2C1_CLK_ENABLE()  PREBENCH_NO_EFFECT()
#define __HAL_RCC_I2C1_CLK_DISABLE() PREBENCH_NO_EFFECT()
#define __HAL_RCC_I2C2_CLK_ENABLE()  PREBENCH_NO_EFFECT()
#define __HAL_RCC_I2C2_CLK_DISABLE() PREBENCH_NO_EFFECT()
#define __HAL_RCC_I2C3_CLK_ENABLE()  PREBENCH_NO_EFFECT()
#define __HAL_RCC_I2C3_CLK_DISABLE() PREBENCH_NO_EFFECT()
#define __HAL_RCC_SPI1_CLK_ENABLE()  PREBENCH_NO_EFFECT()
#define __HAL_RCC_SPI1_CLK_DISABLE() PREBENCH_NO_EFFECT()
#define __HAL_RCC_SPI2_CLK_ENABLE()  PREBENCH_NO_EFFECT()
#define __HAL_RCC_SPI2_CLK_DISABLE() PREBENCH_NO_EFFECT()
#define __HAL_RCC_SPI3_CLK_ENABLE()  PREBENCH_NO_EFFECT()
#define __HAL_RCC_SPI3_CLK_DISABLE() PREBENCH_NO_EFFECT()
#define __HAL_RCC_SPI4_CLK_ENABLE()  PREBENCH_NO_EFFECT()
#define __HAL_RCC_SPI4_CLK_DISABLE() PREBENCH_NO_EFFECT()
#define __HAL_RCC_DMA1_CLK_ENABLE()  PREBENCH_NO_EFFECT()
#define __HAL_RCC_DMA1_CLK_DISABLE() PREBENCH_NO_EFFECT()
#define __HAL_RCC_DMA2_CLK_ENABLE()  PREBENCH_NO_EFFECT()
#define __HAL_RCC_DMA2_CLK_DISABLE() PREBENCH_NO_EFFECT()
#define __HAL_RCC_USART1_FORCE_RESET()   PREBENCH_NO_EFFECT()
#define __HAL_RCC_USART1_RELEASE_RESET() PREBENCH_NO_EFFECT()
#define __HAL_RCC_USART2_FORCE_RESET()   PREBENCH_NO_EFFECT()
#define __HAL_RCC_USART2_RELEASE_RESET() PREBENCH_NO_EFFECT()
#define __HAL_RCC_USART6_FORCE_RESET()   PREBENCH_NO_EFFECT()
#define __HAL_RCC_USART6_RELEASE_RESET() PREBENCH_NO_EFFECT()
#define __HAL_RCC_TIM1_FORCE_RESET()   PREBENCH_NO_EFFECT()
#define __HAL_RCC_TIM1_RELEASE_RESET() PREBENCH_NO_EFFECT()
#define __HAL_RCC_TIM2_FORCE_RESET()   PREBENCH_NO_EFFECT()
#define __HAL_RCC_TIM2_RELEASE_RESET() PREBENCH_NO_EFFECT()
#define __HAL_RCC_TIM3_FORCE_RESET()   PREBENCH_NO_EFFECT()
#define __HAL_RCC_TIM3_RELEASE_RESET() PREBENCH_NO_EFFECT()
#define __HAL_RCC_TIM4_FORCE_RESET()   PREBENCH_NO_EFFECT()
#define __HAL_RCC_TIM4_RELEASE_RESET() PREBENCH_NO_EFFECT()
#define __HAL_RCC_TIM5_FORCE_RESET()   PREBENCH_NO_EFFECT()
#define __HAL_RCC_TIM5_RELEASE_RESET() PREBENCH_NO_EFFECT()
#define __HAL_RCC_I2C1_FORCE_RESET()   PREBENCH_NO_EFFECT()
#define __HAL_RCC_I2C1_RELEASE_RESET() PREBENCH_NO_EFFECT()
#define __HAL_RCC_I2C2_FORCE_RESET()   PREBENCH_NO_EFFECT()
#define __HAL_RCC_I2C2_RELEASE_RESET() PREBENCH_NO_EFFECT()
#define __HAL_RCC_I2C3_FORCE_RESET()   PREBENCH_NO_EFFECT()
#define __HAL_RCC_I2C3_RELEASE_RESET() PREBENCH_NO_EFFECT()
#define __HAL_RCC_SPI1_FORCE_RESET()   PREBENCH_NO_EFFECT()
#define __HAL_RCC_SPI1_RELEASE_RESET() PREBENCH_NO_EFFECT()
#define __HAL_RCC_SPI2_FORCE_RESET()   PREBENCH_NO_EFFECT()
#define __HAL_RCC_SPI2_RELEASE_RESET() PREBENCH_NO_EFFECT()
#define __HAL_RCC_SPI3_FORCE_RESET()   PREBENCH_NO_EFFECT()
#define __HAL_RCC_SPI3_RELEASE_RESET() PREBENCH_NO_EFFECT()

/* --- PWR ------------------------------------------------------------------------------------ */

#define PWR_REGULATOR_VOLTAGE_SCALE2 0x00008000U
#define PWR_REGULATOR_VOLTAGE_SCALE3 0x00004000U
#define __HAL_PWR_VOLTAGESCALING_CONFIG(__REGULATOR__) do { (void)(__REGULATOR__); } while (0)

/* --- Cortex / NVIC ------------------------------------------------------------------------- */

#define NVIC_PRIORITYGROUP_0 0x00000007U
#define NVIC_PRIORITYGROUP_1 0x00000006U
#define NVIC_PRIORITYGROUP_2 0x00000005U
#define NVIC_PRIORITYGROUP_3 0x00000004U
#define NVIC_PRIORITYGROUP_4 0x00000003U
#define SYSTICK_CLKSOURCE_HCLK_DIV8 0x00000000U
#define SYSTICK_CLKSOURCE_HCLK      0x00000004U

void HAL_NVIC_SetPriorityGrouping(uint32_t PriorityGroup);
void HAL_NVIC_SetPriority(IRQn_Type IRQn, uint32_t PreemptPriority, uint32_t SubPriority);
void HAL_NVIC_EnableIRQ(IRQn_Type IRQn);
void HAL_NVIC_DisableIRQ(IRQn_Type IRQn);
void HAL_NVIC_SystemReset(void);
uint32_t HAL_SYSTICK_Config(uint32_t TicksNumb);
void HAL_SYSTICK_CLKSourceConfig(uint32_t CLKSource);

/* --- GPIO ------------------------------------------------------------------------------------ */

typedef struct {
    uint32_t Pin;
    uint32_t Mode;
    uint32_t Pull;
    uint32_t Speed;
    uint32_t Alternate;
} GPIO_InitTypeDef;

typedef enum {
    GPIO_PIN_RESET = 0,
    GPIO_PIN_SET
} GPIO_PinState;

#define GPIO_PIN_0    ((uint16_t)0x0001)
#define GPIO_PIN_1    ((uint16_t)0x0002)
#define GPIO_PIN_2    ((uint16_t)0x0004)
#define GPIO_PIN_3    ((uint16_t)0x0008)
#define GPIO_PIN_4    ((uint16_t)0x0010)
#define GPIO_PIN_5    ((uint16_t)0x0020)
#define GPIO_PIN_6    ((uint16_t)0x0040)
#define GPIO_PIN_7    ((uint16_t)0x0080)
#define GPIO_PIN_8    ((uint16_t)0x0100)
#define GPIO_PIN_9    ((uint16_t)0x0200)
#define GPIO_PIN_10   ((uint16_t)0x0400)
#define GPIO_PIN_11   ((uint16_t)0x0800)
#define GPIO_PIN_12   ((uint16_t)0x1000)
#define GPIO_PIN_13   ((uint16_t)0x2000)
#define GPIO_PIN_14   ((uint16_t)0x4000)
#define GPIO_PIN_15   ((uint16_t)0x8000)
#define GPIO_PIN_All  ((uint16_t)0xFFFF)

#define GPIO_MODE_INPUT              0x00000000U
#define GPIO_MODE_OUTPUT_PP          0x00000001U
#define GPIO_MODE_OUTPUT_OD          0x00000011U
#define GPIO_MODE_AF_PP              0x00000002U
#define GPIO_MODE_AF_OD              0x00000012U
#define GPIO_MODE_ANALOG             0x00000003U
#define GPIO_MODE_IT_RISING          0x10110000U
#define GPIO_MODE_IT_FALLING         0x10210000U
#define GPIO_MODE_IT_RISING_FALLING  0x10310000U
#define GPIO_MODE_EVT_RISING         0x10120000U
#define GPIO_MODE_EVT_FALLING        0x10220000U
#define GPIO_MODE_EVT_RISING_FALLING 0x10320000U

#define GPIO_NOPULL   0x00000000U
#define GPIO_PULLUP   0x00000001U
#define GPIO_PULLDOWN 0x00000002U

#define GPIO_SPEED_FREQ_LOW       0x00000000U
#define GPIO_SPEED_FREQ_MEDIUM    0x00000001U
#define GPIO_SPEED_FREQ_HIGH      0x00000002U
#define GPIO_SPEED_FREQ_VERY_HIGH 0x00000003U

/* Fonctions alternatives STM32F401 (valeur = numéro AF). */
#define GPIO_AF0_RTC_50Hz ((uint8_t)0x00)
#define GPIO_AF0_MCO      ((uint8_t)0x00)
#define GPIO_AF0_TAMPER   ((uint8_t)0x00)
#define GPIO_AF0_SWJ      ((uint8_t)0x00)
#define GPIO_AF0_TRACE    ((uint8_t)0x00)
#define GPIO_AF1_TIM1     ((uint8_t)0x01)
#define GPIO_AF1_TIM2     ((uint8_t)0x01)
#define GPIO_AF2_TIM3     ((uint8_t)0x02)
#define GPIO_AF2_TIM4     ((uint8_t)0x02)
#define GPIO_AF2_TIM5     ((uint8_t)0x02)
#define GPIO_AF3_TIM9     ((uint8_t)0x03)
#define GPIO_AF3_TIM10    ((uint8_t)0x03)
#define GPIO_AF3_TIM11    ((uint8_t)0x03)
#define GPIO_AF4_I2C1     ((uint8_t)0x04)
#define GPIO_AF4_I2C2     ((uint8_t)0x04)
#define GPIO_AF4_I2C3     ((uint8_t)0x04)
#define GPIO_AF5_SPI1     ((uint8_t)0x05)
#define GPIO_AF5_SPI2     ((uint8_t)0x05)
#define GPIO_AF5_SPI3     ((uint8_t)0x05)
#define GPIO_AF5_SPI4     ((uint8_t)0x05)
#define GPIO_AF5_I2S3ext  ((uint8_t)0x05)
#define GPIO_AF6_SPI3     ((uint8_t)0x06)
#define GPIO_AF6_I2S2ext  ((uint8_t)0x06)
#define GPIO_AF7_USART1   ((uint8_t)0x07)
#define GPIO_AF7_USART2   ((uint8_t)0x07)
#define GPIO_AF7_I2S3ext  ((uint8_t)0x07)
#define GPIO_AF8_USART6   ((uint8_t)0x08)
#define GPIO_AF9_I2C2     ((uint8_t)0x09)
#define GPIO_AF9_I2C3     ((uint8_t)0x09)
#define GPIO_AF10_OTG_FS  ((uint8_t)0x0A)
#define GPIO_AF12_SDIO    ((uint8_t)0x0C)
#define GPIO_AF15_EVENTOUT ((uint8_t)0x0F)

void HAL_GPIO_Init(GPIO_TypeDef *GPIOx, GPIO_InitTypeDef *GPIO_Init);
void HAL_GPIO_DeInit(GPIO_TypeDef *GPIOx, uint32_t GPIO_Pin);
GPIO_PinState HAL_GPIO_ReadPin(GPIO_TypeDef *GPIOx, uint16_t GPIO_Pin);
void HAL_GPIO_WritePin(GPIO_TypeDef *GPIOx, uint16_t GPIO_Pin, GPIO_PinState PinState);
void HAL_GPIO_TogglePin(GPIO_TypeDef *GPIOx, uint16_t GPIO_Pin);
HAL_StatusTypeDef HAL_GPIO_LockPin(GPIO_TypeDef *GPIOx, uint16_t GPIO_Pin);
void HAL_GPIO_EXTI_IRQHandler(uint16_t GPIO_Pin);
void HAL_GPIO_EXTI_Callback(uint16_t GPIO_Pin);

/* Drapeaux EXTI « en attente », tenus par le shim. */
uint32_t prebench_exti_get_pending(uint32_t lines);
void prebench_exti_clear_pending(uint32_t lines);
#define __HAL_GPIO_EXTI_GET_IT(__EXTI_LINE__)     prebench_exti_get_pending(__EXTI_LINE__)
#define __HAL_GPIO_EXTI_CLEAR_IT(__EXTI_LINE__)   prebench_exti_clear_pending(__EXTI_LINE__)
#define __HAL_GPIO_EXTI_GET_FLAG(__EXTI_LINE__)   prebench_exti_get_pending(__EXTI_LINE__)
#define __HAL_GPIO_EXTI_CLEAR_FLAG(__EXTI_LINE__) prebench_exti_clear_pending(__EXTI_LINE__)

/* Gestionnaires EXTI : ceux du projet (stm32f4xx_it.c) s'ils existent, sinon
 * ceux du shim. */
void EXTI0_IRQHandler(void);
void EXTI1_IRQHandler(void);
void EXTI2_IRQHandler(void);
void EXTI3_IRQHandler(void);
void EXTI4_IRQHandler(void);
void EXTI9_5_IRQHandler(void);
void EXTI15_10_IRQHandler(void);

#ifdef __cplusplus
}
#endif

/* Modules périphériques (non simulés en phase 1). */
#include "stm32f4xx_hal_dma.h"
#include "stm32f4xx_hal_uart.h"
#include "stm32f4xx_hal_tim.h"
#include "stm32f4xx_hal_adc.h"
#include "stm32f4xx_hal_i2c.h"
#include "stm32f4xx_hal_spi.h"

#endif /* PREBENCH_STM32F4XX_HAL_H */
