/*
 * libvmcu/shims/stm32-hal/include/stm32f4xx.h — En-tête « device » STM32F4 du shim.
 *
 * Remplace l'en-tête CMSIS de ST pour la cible « sim » : mêmes types et noms
 * (IRQn_Type, GPIO_TypeDef, GPIOA…), mais AUCUN registre n'est simulé
 * (règle 16.2.1). Un accès direct comme GPIOA->ODR compile et n'a aucun effet
 * (hors périmètre, section 3.3).
 *
 * Phase 1 : famille STM32F401 (Nucleo-F401RE).
 */
#ifndef PREBENCH_STM32F4XX_H
#define PREBENCH_STM32F4XX_H

#include <stddef.h>
#include <stdint.h>

#include "vmcu.h"

#ifdef __cplusplus
extern "C" {
#endif

#ifndef STM32F401xE
#define STM32F401xE
#endif
#ifndef STM32F4
#define STM32F4
#endif

/* --- Qualificatifs CMSIS --------------------------------------------------------- */

#define __I      volatile const
#define __O      volatile
#define __IO     volatile
#define __IM     volatile const
#define __OM     volatile
#define __IOM    volatile
#ifndef __STATIC_INLINE
#define __STATIC_INLINE static inline
#endif
#ifndef __weak
#define __weak   __attribute__((weak))
#endif
#ifndef __packed
#define __packed __attribute__((packed))
#endif
#ifndef __ALIGN_BEGIN
#define __ALIGN_BEGIN
#endif
#ifndef __ALIGN_END
#define __ALIGN_END __attribute__((aligned(4)))
#endif

/* --- Interruptions (numéros STM32F401) ---------------------------------------------- */

typedef enum {
    NonMaskableInt_IRQn    = -14,
    MemoryManagement_IRQn  = -12,
    BusFault_IRQn          = -11,
    UsageFault_IRQn        = -10,
    SVCall_IRQn            = -5,
    DebugMonitor_IRQn      = -4,
    PendSV_IRQn            = -2,
    SysTick_IRQn           = -1,
    WWDG_IRQn              = 0,
    PVD_IRQn               = 1,
    TAMP_STAMP_IRQn        = 2,
    RTC_WKUP_IRQn          = 3,
    FLASH_IRQn             = 4,
    RCC_IRQn               = 5,
    EXTI0_IRQn             = 6,
    EXTI1_IRQn             = 7,
    EXTI2_IRQn             = 8,
    EXTI3_IRQn             = 9,
    EXTI4_IRQn             = 10,
    DMA1_Stream0_IRQn      = 11,
    DMA1_Stream1_IRQn      = 12,
    DMA1_Stream2_IRQn      = 13,
    DMA1_Stream3_IRQn      = 14,
    DMA1_Stream4_IRQn      = 15,
    DMA1_Stream5_IRQn      = 16,
    DMA1_Stream6_IRQn      = 17,
    ADC_IRQn               = 18,
    EXTI9_5_IRQn           = 23,
    TIM1_BRK_TIM9_IRQn     = 24,
    TIM1_UP_TIM10_IRQn     = 25,
    TIM1_TRG_COM_TIM11_IRQn = 26,
    TIM1_CC_IRQn           = 27,
    TIM2_IRQn              = 28,
    TIM3_IRQn              = 29,
    TIM4_IRQn              = 30,
    I2C1_EV_IRQn           = 31,
    I2C1_ER_IRQn           = 32,
    I2C2_EV_IRQn           = 33,
    I2C2_ER_IRQn           = 34,
    SPI1_IRQn              = 35,
    SPI2_IRQn              = 36,
    USART1_IRQn            = 37,
    USART2_IRQn            = 38,
    EXTI15_10_IRQn         = 40,
    RTC_Alarm_IRQn         = 41,
    OTG_FS_WKUP_IRQn       = 42,
    DMA1_Stream7_IRQn      = 47,
    SDIO_IRQn              = 49,
    TIM5_IRQn              = 50,
    SPI3_IRQn              = 51,
    DMA2_Stream0_IRQn      = 56,
    DMA2_Stream1_IRQn      = 57,
    DMA2_Stream2_IRQn      = 58,
    DMA2_Stream3_IRQn      = 59,
    DMA2_Stream4_IRQn      = 60,
    OTG_FS_IRQn            = 67,
    DMA2_Stream5_IRQn      = 68,
    DMA2_Stream6_IRQn      = 69,
    DMA2_Stream7_IRQn      = 70,
    USART6_IRQn            = 71,
    I2C3_EV_IRQn           = 72,
    I2C3_ER_IRQn           = 73,
    FPU_IRQn               = 81,
    SPI4_IRQn              = 84
} IRQn_Type;

/* --- GPIO --------------------------------------------------------------------------- */

/* Disposition des registres conservée pour la compatibilité des sources ;
 * le contenu n'est ni lu ni écrit par la simulation. */
typedef struct {
    __IO uint32_t MODER;
    __IO uint32_t OTYPER;
    __IO uint32_t OSPEEDR;
    __IO uint32_t PUPDR;
    __IO uint32_t IDR;
    __IO uint32_t ODR;
    __IO uint32_t BSRR;
    __IO uint32_t LCKR;
    __IO uint32_t AFR[2];
} GPIO_TypeDef;

#define PREBENCH_GPIO_PORT_COUNT 8u /* A..H ; l'index est le `port` de vmcu_* */
extern GPIO_TypeDef prebench_gpio_ports[PREBENCH_GPIO_PORT_COUNT];

#define GPIOA (&prebench_gpio_ports[0])
#define GPIOB (&prebench_gpio_ports[1])
#define GPIOC (&prebench_gpio_ports[2])
#define GPIOD (&prebench_gpio_ports[3])
#define GPIOE (&prebench_gpio_ports[4])
#define GPIOH (&prebench_gpio_ports[7])

/* --- Autres périphériques ---------------------------------------------------------- */
/* Mêmes noms de registres que CMSIS, pour que les sources compilent ; seuls
 * certains champs servent de stockage aux macros du HAL (CCRx, CNT, ARR, NDTR). */

typedef struct {
    __IO uint32_t SR, DR, BRR, CR1, CR2, CR3, GTPR;
} USART_TypeDef;

typedef struct {
    __IO uint32_t CR1, CR2, SMCR, DIER, SR, EGR, CCMR1, CCMR2, CCER, CNT, PSC, ARR, RCR;
    __IO uint32_t CCR1, CCR2, CCR3, CCR4, BDTR, DCR, DMAR, OR;
} TIM_TypeDef;

typedef struct {
    __IO uint32_t SR, CR1, CR2, SMPR1, SMPR2, JOFR1, JOFR2, JOFR3, JOFR4, HTR, LTR;
    __IO uint32_t SQR1, SQR2, SQR3, JSQR, JDR1, JDR2, JDR3, JDR4, DR;
} ADC_TypeDef;

typedef struct {
    __IO uint32_t CR1, CR2, OAR1, OAR2, DR, SR1, SR2, CCR, TRISE, FLTR;
} I2C_TypeDef;

typedef struct {
    __IO uint32_t CR1, CR2, SR, DR, CRCPR, RXCRCR, TXCRCR, I2SCFGR, I2SPR;
} SPI_TypeDef;

typedef struct {
    __IO uint32_t CR, NDTR, PAR, M0AR, M1AR, FCR;
} DMA_Stream_TypeDef;

typedef struct {
    __IO uint32_t LISR, HISR, LIFCR, HIFCR;
} DMA_TypeDef;

/* Instances : l'index du tableau est le numéro du périphérique (USART2 -> 2). */
extern USART_TypeDef      prebench_usart[7];
extern TIM_TypeDef        prebench_tim[12];
extern ADC_TypeDef        prebench_adc[2];
extern I2C_TypeDef        prebench_i2c[4];
extern SPI_TypeDef        prebench_spi[5];
extern DMA_TypeDef        prebench_dma[3];
extern DMA_Stream_TypeDef prebench_dma_stream[16]; /* DMA1 : 0..7, DMA2 : 8..15 */

#define USART1 (&prebench_usart[1])
#define USART2 (&prebench_usart[2])
#define USART6 (&prebench_usart[6])
#define TIM1   (&prebench_tim[1])
#define TIM2   (&prebench_tim[2])
#define TIM3   (&prebench_tim[3])
#define TIM4   (&prebench_tim[4])
#define TIM5   (&prebench_tim[5])
#define TIM9   (&prebench_tim[9])
#define TIM10  (&prebench_tim[10])
#define TIM11  (&prebench_tim[11])
#define ADC1   (&prebench_adc[1])
#define I2C1   (&prebench_i2c[1])
#define I2C2   (&prebench_i2c[2])
#define I2C3   (&prebench_i2c[3])
#define SPI1   (&prebench_spi[1])
#define SPI2   (&prebench_spi[2])
#define SPI3   (&prebench_spi[3])
#define SPI4   (&prebench_spi[4])
#define DMA1   (&prebench_dma[1])
#define DMA2   (&prebench_dma[2])
#define DMA1_Stream0 (&prebench_dma_stream[0])
#define DMA1_Stream1 (&prebench_dma_stream[1])
#define DMA1_Stream2 (&prebench_dma_stream[2])
#define DMA1_Stream3 (&prebench_dma_stream[3])
#define DMA1_Stream4 (&prebench_dma_stream[4])
#define DMA1_Stream5 (&prebench_dma_stream[5])
#define DMA1_Stream6 (&prebench_dma_stream[6])
#define DMA1_Stream7 (&prebench_dma_stream[7])
#define DMA2_Stream0 (&prebench_dma_stream[8])
#define DMA2_Stream1 (&prebench_dma_stream[9])
#define DMA2_Stream2 (&prebench_dma_stream[10])
#define DMA2_Stream3 (&prebench_dma_stream[11])
#define DMA2_Stream4 (&prebench_dma_stream[12])
#define DMA2_Stream5 (&prebench_dma_stream[13])
#define DMA2_Stream6 (&prebench_dma_stream[14])
#define DMA2_Stream7 (&prebench_dma_stream[15])

/* --- Système -------------------------------------------------------------------------- */

extern uint32_t SystemCoreClock;
void SystemInit(void);
void SystemCoreClockUpdate(void);

/* --- Intrinsèques CMSIS ------------------------------------------------------------------ */

/* Masquage des interruptions : verrou d'interruption de libvmcu. */
void __disable_irq(void);
void __enable_irq(void);
uint32_t __get_PRIMASK(void);
void __set_PRIMASK(uint32_t primask);
/* Attente d'interruption : sommeil court, jamais d'attente active. */
void __WFI(void);
void __WFE(void);

__STATIC_INLINE void __NOP(void) {}
__STATIC_INLINE void __DSB(void) {}
__STATIC_INLINE void __ISB(void) {}
__STATIC_INLINE void __DMB(void) {}

void NVIC_SystemReset(void);

#ifdef __cplusplus
}
#endif

#endif /* PREBENCH_STM32F4XX_H */
