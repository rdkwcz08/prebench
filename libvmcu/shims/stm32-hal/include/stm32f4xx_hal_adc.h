/*
 * libvmcu/shims/stm32-hal/include/stm32f4xx_hal_adc.h — Module ADC du shim.
 *
 * Simulé en phase 2 (src/stm32_hal_adc.c) : les conversions se terminent
 * aussitôt et HAL_ADC_GetValue lit la tension de la broche sur la paillasse
 * (potentiomètre, source analogique). DMA non simulé (compile et avertit).
 */
#ifndef PREBENCH_STM32F4XX_HAL_ADC_H
#define PREBENCH_STM32F4XX_HAL_ADC_H

#include "stm32f4xx_hal_dma.h"

#ifdef __cplusplus
extern "C" {
#endif

typedef struct {
    uint32_t        ClockPrescaler;
    uint32_t        Resolution;
    uint32_t        DataAlign;
    uint32_t        ScanConvMode;
    uint32_t        EOCSelection;
    FunctionalState ContinuousConvMode;
    uint32_t        NbrOfConversion;
    FunctionalState DiscontinuousConvMode;
    uint32_t        NbrOfDiscConversion;
    uint32_t        ExternalTrigConv;
    uint32_t        ExternalTrigConvEdge;
    FunctionalState DMAContinuousRequests;
} ADC_InitTypeDef;

typedef struct {
    uint32_t Channel;
    uint32_t Rank;
    uint32_t SamplingTime;
    uint32_t Offset;
} ADC_ChannelConfTypeDef;

typedef struct {
    uint32_t        WatchdogMode;
    uint32_t        HighThreshold;
    uint32_t        LowThreshold;
    uint32_t        Channel;
    FunctionalState ITMode;
    uint32_t        WatchdogNumber;
} ADC_AnalogWDGConfTypeDef;

typedef struct __ADC_HandleTypeDef {
    ADC_TypeDef         *Instance;
    ADC_InitTypeDef      Init;
    __IO uint32_t        NbrOfCurrentConversionRank;
    DMA_HandleTypeDef   *DMA_Handle;
    HAL_LockTypeDef      Lock;
    __IO uint32_t        State;
    __IO uint32_t        ErrorCode;
} ADC_HandleTypeDef;

#define HAL_ADC_STATE_RESET          0x00000000U
#define HAL_ADC_STATE_READY          0x00000001U
#define HAL_ADC_STATE_BUSY_INTERNAL  0x00000002U
#define HAL_ADC_STATE_TIMEOUT        0x00000004U
#define HAL_ADC_STATE_ERROR_INTERNAL 0x00000010U
#define HAL_ADC_STATE_ERROR_CONFIG   0x00000020U
#define HAL_ADC_STATE_ERROR_DMA      0x00000040U
#define HAL_ADC_STATE_REG_BUSY       0x00000100U
#define HAL_ADC_STATE_REG_EOC        0x00000200U
#define HAL_ADC_STATE_REG_OVR        0x00000400U
#define HAL_ADC_STATE_INJ_BUSY       0x00001000U
#define HAL_ADC_STATE_INJ_EOC        0x00002000U
#define HAL_ADC_STATE_AWD1           0x00010000U

#define HAL_ADC_ERROR_NONE     0x00U
#define HAL_ADC_ERROR_INTERNAL 0x01U
#define HAL_ADC_ERROR_OVR      0x02U
#define HAL_ADC_ERROR_DMA      0x04U

#define ADC_CLOCK_SYNC_PCLK_DIV2 0x00000000U
#define ADC_CLOCK_SYNC_PCLK_DIV4 0x00010000U
#define ADC_CLOCK_SYNC_PCLK_DIV6 0x00020000U
#define ADC_CLOCK_SYNC_PCLK_DIV8 0x00030000U
#define ADC_RESOLUTION_12B 0x00000000U
#define ADC_RESOLUTION_10B 0x01000000U
#define ADC_RESOLUTION_8B  0x02000000U
#define ADC_RESOLUTION_6B  0x03000000U
#define ADC_DATAALIGN_RIGHT 0x00000000U
#define ADC_DATAALIGN_LEFT  0x00000800U
#define ADC_EXTERNALTRIGCONVEDGE_NONE          0x00000000U
#define ADC_EXTERNALTRIGCONVEDGE_RISING        0x10000000U
#define ADC_EXTERNALTRIGCONVEDGE_FALLING       0x20000000U
#define ADC_EXTERNALTRIGCONVEDGE_RISINGFALLING 0x30000000U
#define ADC_EXTERNALTRIGCONV_T1_CC1  0x00000000U
#define ADC_EXTERNALTRIGCONV_T1_CC2  0x01000000U
#define ADC_EXTERNALTRIGCONV_T1_CC3  0x02000000U
#define ADC_EXTERNALTRIGCONV_T2_CC2  0x03000000U
#define ADC_EXTERNALTRIGCONV_T2_CC3  0x04000000U
#define ADC_EXTERNALTRIGCONV_T2_CC4  0x05000000U
#define ADC_EXTERNALTRIGCONV_T2_TRGO 0x06000000U
#define ADC_EXTERNALTRIGCONV_T3_CC1  0x07000000U
#define ADC_EXTERNALTRIGCONV_T3_TRGO 0x08000000U
#define ADC_EXTERNALTRIGCONV_T4_CC4  0x09000000U
#define ADC_EXTERNALTRIGCONV_T5_CC1  0x0A000000U
#define ADC_EXTERNALTRIGCONV_T5_CC2  0x0B000000U
#define ADC_EXTERNALTRIGCONV_T5_CC3  0x0C000000U
#define ADC_EXTERNALTRIGCONV_Ext_IT11 0x0F000000U
#define ADC_SOFTWARE_START 0x0F000001U
#define ADC_EOC_SEQ_CONV        0x00000000U
#define ADC_EOC_SINGLE_CONV     0x00000001U
#define ADC_EOC_SINGLE_SEQ_CONV 0x00000002U

#define ADC_CHANNEL_0  0x00000000U
#define ADC_CHANNEL_1  0x00000001U
#define ADC_CHANNEL_2  0x00000002U
#define ADC_CHANNEL_3  0x00000003U
#define ADC_CHANNEL_4  0x00000004U
#define ADC_CHANNEL_5  0x00000005U
#define ADC_CHANNEL_6  0x00000006U
#define ADC_CHANNEL_7  0x00000007U
#define ADC_CHANNEL_8  0x00000008U
#define ADC_CHANNEL_9  0x00000009U
#define ADC_CHANNEL_10 0x0000000AU
#define ADC_CHANNEL_11 0x0000000BU
#define ADC_CHANNEL_12 0x0000000CU
#define ADC_CHANNEL_13 0x0000000DU
#define ADC_CHANNEL_14 0x0000000EU
#define ADC_CHANNEL_15 0x0000000FU
#define ADC_CHANNEL_16 0x00000010U
#define ADC_CHANNEL_17 0x00000011U
#define ADC_CHANNEL_18 0x00000012U
#define ADC_CHANNEL_VREFINT    ADC_CHANNEL_17
#define ADC_CHANNEL_VBAT       ADC_CHANNEL_18
#define ADC_CHANNEL_TEMPSENSOR ADC_CHANNEL_18
#define ADC_SAMPLETIME_3CYCLES   0x00000000U
#define ADC_SAMPLETIME_15CYCLES  0x00000001U
#define ADC_SAMPLETIME_28CYCLES  0x00000002U
#define ADC_SAMPLETIME_56CYCLES  0x00000003U
#define ADC_SAMPLETIME_84CYCLES  0x00000004U
#define ADC_SAMPLETIME_112CYCLES 0x00000005U
#define ADC_SAMPLETIME_144CYCLES 0x00000006U
#define ADC_SAMPLETIME_480CYCLES 0x00000007U
#define ADC_ANALOGWATCHDOG_NONE             0x00000000U
#define ADC_ANALOGWATCHDOG_SINGLE_REG       0x00800200U
#define ADC_ANALOGWATCHDOG_ALL_REG          0x00800000U
#define ADC_AWD_EVENT 0x00000001U
#define ADC_OVR_EVENT 0x00000020U
#define ADC_FLAG_AWD   0x00000001U
#define ADC_FLAG_EOC   0x00000002U
#define ADC_FLAG_JEOC  0x00000004U
#define ADC_FLAG_JSTRT 0x00000008U
#define ADC_FLAG_STRT  0x00000010U
#define ADC_FLAG_OVR   0x00000020U
#define ADC_IT_EOC  0x00000020U
#define ADC_IT_AWD  0x00000040U
#define ADC_IT_JEOC 0x00000080U
#define ADC_IT_OVR  0x04000000U

#define __HAL_ADC_ENABLE(__HANDLE__)  ((void)(__HANDLE__))
#define __HAL_ADC_DISABLE(__HANDLE__) ((void)(__HANDLE__))
#define __HAL_ADC_ENABLE_IT(__HANDLE__, __INTERRUPT__)  ((void)(__HANDLE__), (void)(__INTERRUPT__))
#define __HAL_ADC_DISABLE_IT(__HANDLE__, __INTERRUPT__) ((void)(__HANDLE__), (void)(__INTERRUPT__))
/* Conversion terminée aussitôt : EOC toujours levé. */
#define __HAL_ADC_GET_FLAG(__HANDLE__, __FLAG__)   ((void)(__HANDLE__), ((__FLAG__) == ADC_FLAG_EOC) ? SET : RESET)
#define __HAL_ADC_CLEAR_FLAG(__HANDLE__, __FLAG__) ((void)(__HANDLE__), (void)(__FLAG__))

HAL_StatusTypeDef HAL_ADC_Init(ADC_HandleTypeDef *hadc);
HAL_StatusTypeDef HAL_ADC_DeInit(ADC_HandleTypeDef *hadc);
void HAL_ADC_MspInit(ADC_HandleTypeDef *hadc);
void HAL_ADC_MspDeInit(ADC_HandleTypeDef *hadc);
HAL_StatusTypeDef HAL_ADC_Start(ADC_HandleTypeDef *hadc);
HAL_StatusTypeDef HAL_ADC_Stop(ADC_HandleTypeDef *hadc);
HAL_StatusTypeDef HAL_ADC_PollForConversion(ADC_HandleTypeDef *hadc, uint32_t Timeout);
HAL_StatusTypeDef HAL_ADC_PollForEvent(ADC_HandleTypeDef *hadc, uint32_t EventType, uint32_t Timeout);
HAL_StatusTypeDef HAL_ADC_Start_IT(ADC_HandleTypeDef *hadc);
HAL_StatusTypeDef HAL_ADC_Stop_IT(ADC_HandleTypeDef *hadc);
void HAL_ADC_IRQHandler(ADC_HandleTypeDef *hadc);
HAL_StatusTypeDef HAL_ADC_Start_DMA(ADC_HandleTypeDef *hadc, uint32_t *pData, uint32_t Length);
HAL_StatusTypeDef HAL_ADC_Stop_DMA(ADC_HandleTypeDef *hadc);
uint32_t HAL_ADC_GetValue(const ADC_HandleTypeDef *hadc);
HAL_StatusTypeDef HAL_ADC_ConfigChannel(ADC_HandleTypeDef *hadc, const ADC_ChannelConfTypeDef *sConfig);
HAL_StatusTypeDef HAL_ADC_AnalogWDGConfig(ADC_HandleTypeDef *hadc, const ADC_AnalogWDGConfTypeDef *AnalogWDGConfig);
uint32_t HAL_ADC_GetState(const ADC_HandleTypeDef *hadc);
uint32_t HAL_ADC_GetError(const ADC_HandleTypeDef *hadc);

void HAL_ADC_ConvCpltCallback(ADC_HandleTypeDef *hadc);
void HAL_ADC_ConvHalfCpltCallback(ADC_HandleTypeDef *hadc);
void HAL_ADC_LevelOutOfWindowCallback(ADC_HandleTypeDef *hadc);
void HAL_ADC_ErrorCallback(ADC_HandleTypeDef *hadc);
void HAL_ADCEx_InjectedConvCpltCallback(ADC_HandleTypeDef *hadc);

#ifdef __cplusplus
}
#endif

#endif /* PREBENCH_STM32F4XX_HAL_ADC_H */
