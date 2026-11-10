/*
 * libvmcu/shims/stm32-hal/include/stm32f4xx_hal_tim.h — Module TIM du shim.
 *
 * Phase 1 : non simulé. Les fonctions *_Init appellent leur MspInit comme le
 * vrai HAL ; aucun compteur ne tourne et aucun callback n'est appelé. La PWM
 * (rapport cyclique via __HAL_TIM_SET_COMPARE) est simulée en phase 2.
 * Les macros __HAL_TIM_* passent par des fonctions du shim pour que la phase 2
 * puisse les simuler sans changer les sources.
 */
#ifndef PREBENCH_STM32F4XX_HAL_TIM_H
#define PREBENCH_STM32F4XX_HAL_TIM_H

#include "stm32f4xx_hal_dma.h"

#ifdef __cplusplus
extern "C" {
#endif

typedef struct {
    uint32_t Prescaler;
    uint32_t CounterMode;
    uint32_t Period;
    uint32_t ClockDivision;
    uint32_t RepetitionCounter;
    uint32_t AutoReloadPreload;
} TIM_Base_InitTypeDef;

typedef struct {
    uint32_t OCMode;
    uint32_t Pulse;
    uint32_t OCPolarity;
    uint32_t OCNPolarity;
    uint32_t OCFastMode;
    uint32_t OCIdleState;
    uint32_t OCNIdleState;
} TIM_OC_InitTypeDef;

typedef struct {
    uint32_t OCMode;
    uint32_t Pulse;
    uint32_t OCPolarity;
    uint32_t OCNPolarity;
    uint32_t OCIdleState;
    uint32_t OCNIdleState;
    uint32_t ICPolarity;
    uint32_t ICSelection;
    uint32_t ICFilter;
} TIM_OnePulse_InitTypeDef;

typedef struct {
    uint32_t ICPolarity;
    uint32_t ICSelection;
    uint32_t ICPrescaler;
    uint32_t ICFilter;
} TIM_IC_InitTypeDef;

typedef struct {
    uint32_t EncoderMode;
    uint32_t IC1Polarity;
    uint32_t IC1Selection;
    uint32_t IC1Prescaler;
    uint32_t IC1Filter;
    uint32_t IC2Polarity;
    uint32_t IC2Selection;
    uint32_t IC2Prescaler;
    uint32_t IC2Filter;
} TIM_Encoder_InitTypeDef;

typedef struct {
    uint32_t ClockSource;
    uint32_t ClockPolarity;
    uint32_t ClockPrescaler;
    uint32_t ClockFilter;
} TIM_ClockConfigTypeDef;

typedef struct {
    uint32_t MasterOutputTrigger;
    uint32_t MasterSlaveMode;
} TIM_MasterConfigTypeDef;

typedef struct {
    uint32_t SlaveMode;
    uint32_t InputTrigger;
    uint32_t TriggerPolarity;
    uint32_t TriggerPrescaler;
    uint32_t TriggerFilter;
} TIM_SlaveConfigTypeDef;

typedef struct {
    uint32_t OffStateRunMode;
    uint32_t OffStateIDLEMode;
    uint32_t LockLevel;
    uint32_t DeadTime;
    uint32_t BreakState;
    uint32_t BreakPolarity;
    uint32_t BreakFilter;
    uint32_t AutomaticOutput;
} TIM_BreakDeadTimeConfigTypeDef;

typedef enum {
    HAL_TIM_STATE_RESET   = 0x00U,
    HAL_TIM_STATE_READY   = 0x01U,
    HAL_TIM_STATE_BUSY    = 0x02U,
    HAL_TIM_STATE_TIMEOUT = 0x03U,
    HAL_TIM_STATE_ERROR   = 0x04U
} HAL_TIM_StateTypeDef;

typedef enum {
    HAL_TIM_CHANNEL_STATE_RESET = 0x00U,
    HAL_TIM_CHANNEL_STATE_READY = 0x01U,
    HAL_TIM_CHANNEL_STATE_BUSY  = 0x02U
} HAL_TIM_ChannelStateTypeDef;

typedef enum {
    HAL_TIM_ACTIVE_CHANNEL_1       = 0x01U,
    HAL_TIM_ACTIVE_CHANNEL_2       = 0x02U,
    HAL_TIM_ACTIVE_CHANNEL_3       = 0x04U,
    HAL_TIM_ACTIVE_CHANNEL_4       = 0x08U,
    HAL_TIM_ACTIVE_CHANNEL_CLEARED = 0x00U
} HAL_TIM_ActiveChannel;

typedef struct __TIM_HandleTypeDef {
    TIM_TypeDef                       *Instance;
    TIM_Base_InitTypeDef               Init;
    HAL_TIM_ActiveChannel              Channel;
    DMA_HandleTypeDef                 *hdma[7];
    HAL_LockTypeDef                    Lock;
    __IO HAL_TIM_StateTypeDef          State;
    __IO HAL_TIM_ChannelStateTypeDef   ChannelState[4];
    __IO HAL_TIM_ChannelStateTypeDef   ChannelNState[4];
} TIM_HandleTypeDef;

#define TIM_CHANNEL_1   0x00000000U
#define TIM_CHANNEL_2   0x00000004U
#define TIM_CHANNEL_3   0x00000008U
#define TIM_CHANNEL_4   0x0000000CU
#define TIM_CHANNEL_ALL 0x0000003CU

#define TIM_COUNTERMODE_UP             0x00000000U
#define TIM_COUNTERMODE_DOWN           0x00000010U
#define TIM_COUNTERMODE_CENTERALIGNED1 0x00000020U
#define TIM_COUNTERMODE_CENTERALIGNED2 0x00000040U
#define TIM_COUNTERMODE_CENTERALIGNED3 0x00000060U
#define TIM_CLOCKDIVISION_DIV1 0x00000000U
#define TIM_CLOCKDIVISION_DIV2 0x00000100U
#define TIM_CLOCKDIVISION_DIV4 0x00000200U
#define TIM_AUTORELOAD_PRELOAD_DISABLE 0x00000000U
#define TIM_AUTORELOAD_PRELOAD_ENABLE  0x00000080U

#define TIM_CLOCKSOURCE_ETRMODE2 0x00002000U
#define TIM_CLOCKSOURCE_INTERNAL 0x00001000U
#define TIM_CLOCKSOURCE_ITR0     0x00000000U
#define TIM_CLOCKSOURCE_ITR1     0x00000010U
#define TIM_CLOCKSOURCE_ITR2     0x00000020U
#define TIM_CLOCKSOURCE_ITR3     0x00000030U
#define TIM_CLOCKSOURCE_TI1ED    0x00000040U
#define TIM_CLOCKSOURCE_TI1      0x00000050U
#define TIM_CLOCKSOURCE_TI2      0x00000060U
#define TIM_CLOCKSOURCE_ETRMODE1 0x00000070U
#define TIM_CLOCKPOLARITY_INVERTED    0x00008000U
#define TIM_CLOCKPOLARITY_NONINVERTED 0x00000000U
#define TIM_CLOCKPOLARITY_RISING      0x00000000U
#define TIM_CLOCKPOLARITY_FALLING     0x00000002U
#define TIM_CLOCKPOLARITY_BOTHEDGE    0x0000000AU
#define TIM_CLOCKPRESCALER_DIV1 0x00000000U
#define TIM_CLOCKPRESCALER_DIV2 0x00001000U
#define TIM_CLOCKPRESCALER_DIV4 0x00002000U
#define TIM_CLOCKPRESCALER_DIV8 0x00003000U

#define TIM_OCMODE_TIMING          0x00000000U
#define TIM_OCMODE_ACTIVE          0x00000010U
#define TIM_OCMODE_INACTIVE        0x00000020U
#define TIM_OCMODE_TOGGLE          0x00000030U
#define TIM_OCMODE_FORCED_INACTIVE 0x00000040U
#define TIM_OCMODE_FORCED_ACTIVE   0x00000050U
#define TIM_OCMODE_PWM1            0x00000060U
#define TIM_OCMODE_PWM2            0x00000070U
#define TIM_OCPOLARITY_HIGH   0x00000000U
#define TIM_OCPOLARITY_LOW    0x00000002U
#define TIM_OCNPOLARITY_HIGH  0x00000000U
#define TIM_OCNPOLARITY_LOW   0x00000008U
#define TIM_OCFAST_DISABLE    0x00000000U
#define TIM_OCFAST_ENABLE     0x00000004U
#define TIM_OCIDLESTATE_SET    0x00000100U
#define TIM_OCIDLESTATE_RESET  0x00000000U
#define TIM_OCNIDLESTATE_SET   0x00000200U
#define TIM_OCNIDLESTATE_RESET 0x00000000U

#define TIM_ICPOLARITY_RISING   0x00000000U
#define TIM_ICPOLARITY_FALLING  0x00000002U
#define TIM_ICPOLARITY_BOTHEDGE 0x0000000AU
#define TIM_ICSELECTION_DIRECTTI   0x00000001U
#define TIM_ICSELECTION_INDIRECTTI 0x00000002U
#define TIM_ICSELECTION_TRC        0x00000003U
#define TIM_ICPSC_DIV1 0x00000000U
#define TIM_ICPSC_DIV2 0x00000004U
#define TIM_ICPSC_DIV4 0x00000008U
#define TIM_ICPSC_DIV8 0x0000000CU

#define TIM_ENCODERMODE_TI1  0x00000001U
#define TIM_ENCODERMODE_TI2  0x00000002U
#define TIM_ENCODERMODE_TI12 0x00000003U

#define TIM_TRGO_RESET  0x00000000U
#define TIM_TRGO_ENABLE 0x00000010U
#define TIM_TRGO_UPDATE 0x00000020U
#define TIM_TRGO_OC1    0x00000030U
#define TIM_TRGO_OC1REF 0x00000040U
#define TIM_TRGO_OC2REF 0x00000050U
#define TIM_TRGO_OC3REF 0x00000060U
#define TIM_TRGO_OC4REF 0x00000070U
#define TIM_MASTERSLAVEMODE_ENABLE  0x00000080U
#define TIM_MASTERSLAVEMODE_DISABLE 0x00000000U

#define TIM_SLAVEMODE_DISABLE   0x00000000U
#define TIM_SLAVEMODE_RESET     0x00000004U
#define TIM_SLAVEMODE_GATED     0x00000005U
#define TIM_SLAVEMODE_TRIGGER   0x00000006U
#define TIM_SLAVEMODE_EXTERNAL1 0x00000007U
#define TIM_TS_ITR0    0x00000000U
#define TIM_TS_ITR1    0x00000010U
#define TIM_TS_ITR2    0x00000020U
#define TIM_TS_ITR3    0x00000030U
#define TIM_TS_TI1F_ED 0x00000040U
#define TIM_TS_TI1FP1  0x00000050U
#define TIM_TS_TI2FP2  0x00000060U
#define TIM_TS_ETRF    0x00000070U
#define TIM_TS_NONE    0x0000FFFFU
#define TIM_TRIGGERPOLARITY_INVERTED    0x00008000U
#define TIM_TRIGGERPOLARITY_NONINVERTED 0x00000000U
#define TIM_TRIGGERPOLARITY_RISING      0x00000000U
#define TIM_TRIGGERPOLARITY_FALLING     0x00000002U
#define TIM_TRIGGERPOLARITY_BOTHEDGE    0x0000000AU
#define TIM_TRIGGERPRESCALER_DIV1 0x00000000U
#define TIM_TRIGGERPRESCALER_DIV2 0x00001000U
#define TIM_TRIGGERPRESCALER_DIV4 0x00002000U
#define TIM_TRIGGERPRESCALER_DIV8 0x00003000U

#define TIM_OSSR_ENABLE  0x00000800U
#define TIM_OSSR_DISABLE 0x00000000U
#define TIM_OSSI_ENABLE  0x00000400U
#define TIM_OSSI_DISABLE 0x00000000U
#define TIM_LOCKLEVEL_OFF 0x00000000U
#define TIM_LOCKLEVEL_1   0x00000100U
#define TIM_LOCKLEVEL_2   0x00000200U
#define TIM_LOCKLEVEL_3   0x00000300U
#define TIM_BREAK_ENABLE  0x00001000U
#define TIM_BREAK_DISABLE 0x00000000U
#define TIM_BREAKPOLARITY_LOW  0x00000000U
#define TIM_BREAKPOLARITY_HIGH 0x00002000U
#define TIM_AUTOMATICOUTPUT_ENABLE  0x00004000U
#define TIM_AUTOMATICOUTPUT_DISABLE 0x00000000U

#define TIM_IT_UPDATE  0x00000001U
#define TIM_IT_CC1     0x00000002U
#define TIM_IT_CC2     0x00000004U
#define TIM_IT_CC3     0x00000008U
#define TIM_IT_CC4     0x00000010U
#define TIM_IT_COM     0x00000020U
#define TIM_IT_TRIGGER 0x00000040U
#define TIM_IT_BREAK   0x00000080U
#define TIM_FLAG_UPDATE  0x00000001U
#define TIM_FLAG_CC1     0x00000002U
#define TIM_FLAG_CC2     0x00000004U
#define TIM_FLAG_CC3     0x00000008U
#define TIM_FLAG_CC4     0x00000010U
#define TIM_FLAG_COM     0x00000020U
#define TIM_FLAG_TRIGGER 0x00000040U
#define TIM_FLAG_BREAK   0x00000080U
#define TIM_EVENTSOURCE_UPDATE 0x00000001U
#define TIM_EVENTSOURCE_CC1    0x00000002U
#define TIM_EVENTSOURCE_CC2    0x00000004U
#define TIM_EVENTSOURCE_CC3    0x00000008U
#define TIM_EVENTSOURCE_CC4    0x00000010U

#define TIM_TIM11_GPIO  0x00000000U
#define TIM_TIM11_HSE   0x00000002U
#define TIM_TIM5_GPIO   0x00000000U
#define TIM_TIM5_LSI    0x00000040U
#define TIM_TIM5_LSE    0x00000080U
#define TIM_TIM5_RTC    0x000000C0U

/* Macros : registres de stockage + passage par le shim (phase 2 : PWM). */
void     prebench_tim_set_compare(TIM_HandleTypeDef *htim, uint32_t channel, uint32_t compare);
uint32_t prebench_tim_get_compare(const TIM_HandleTypeDef *htim, uint32_t channel);
void     prebench_tim_set_autoreload(TIM_HandleTypeDef *htim, uint32_t autoreload);
void     prebench_tim_set_prescaler(TIM_HandleTypeDef *htim, uint32_t prescaler);
void     prebench_tim_set_counter(TIM_HandleTypeDef *htim, uint32_t counter);
uint32_t prebench_tim_get_counter(const TIM_HandleTypeDef *htim);

#define __HAL_TIM_SET_COMPARE(__HANDLE__, __CHANNEL__, __COMPARE__) \
    prebench_tim_set_compare((__HANDLE__), (__CHANNEL__), (__COMPARE__))
#define __HAL_TIM_GET_COMPARE(__HANDLE__, __CHANNEL__) prebench_tim_get_compare((__HANDLE__), (__CHANNEL__))
#define __HAL_TIM_SET_AUTORELOAD(__HANDLE__, __AUTORELOAD__) prebench_tim_set_autoreload((__HANDLE__), (__AUTORELOAD__))
#define __HAL_TIM_GET_AUTORELOAD(__HANDLE__) ((__HANDLE__)->Instance->ARR)
#define __HAL_TIM_SET_PRESCALER(__HANDLE__, __PRESC__) prebench_tim_set_prescaler((__HANDLE__), (__PRESC__))
#define __HAL_TIM_SET_COUNTER(__HANDLE__, __COUNTER__) prebench_tim_set_counter((__HANDLE__), (__COUNTER__))
#define __HAL_TIM_GET_COUNTER(__HANDLE__) prebench_tim_get_counter((__HANDLE__))
#define __HAL_TIM_SET_CLOCKDIVISION(__HANDLE__, __CKD__) ((__HANDLE__)->Init.ClockDivision = (__CKD__))
#define __HAL_TIM_GET_CLOCKDIVISION(__HANDLE__) ((__HANDLE__)->Init.ClockDivision)
#define __HAL_TIM_ENABLE(__HANDLE__)  ((void)(__HANDLE__))
#define __HAL_TIM_DISABLE(__HANDLE__) ((void)(__HANDLE__))
#define __HAL_TIM_MOE_ENABLE(__HANDLE__)  ((void)(__HANDLE__))
#define __HAL_TIM_MOE_DISABLE(__HANDLE__) ((void)(__HANDLE__))
#define __HAL_TIM_ENABLE_IT(__HANDLE__, __INTERRUPT__)  ((void)(__HANDLE__), (void)(__INTERRUPT__))
#define __HAL_TIM_DISABLE_IT(__HANDLE__, __INTERRUPT__) ((void)(__HANDLE__), (void)(__INTERRUPT__))
#define __HAL_TIM_GET_FLAG(__HANDLE__, __FLAG__)    ((void)(__HANDLE__), (void)(__FLAG__), RESET)
#define __HAL_TIM_CLEAR_FLAG(__HANDLE__, __FLAG__)  ((void)(__HANDLE__), (void)(__FLAG__))
#define __HAL_TIM_GET_IT_SOURCE(__HANDLE__, __IT__) ((void)(__HANDLE__), (void)(__IT__), RESET)
#define __HAL_TIM_CLEAR_IT(__HANDLE__, __INTERRUPT__) ((void)(__HANDLE__), (void)(__INTERRUPT__))
#define __HAL_TIM_URS_ENABLE(__HANDLE__)  ((void)(__HANDLE__))
#define __HAL_TIM_URS_DISABLE(__HANDLE__) ((void)(__HANDLE__))

/* Base */
HAL_StatusTypeDef HAL_TIM_Base_Init(TIM_HandleTypeDef *htim);
HAL_StatusTypeDef HAL_TIM_Base_DeInit(TIM_HandleTypeDef *htim);
void HAL_TIM_Base_MspInit(TIM_HandleTypeDef *htim);
void HAL_TIM_Base_MspDeInit(TIM_HandleTypeDef *htim);
HAL_StatusTypeDef HAL_TIM_Base_Start(TIM_HandleTypeDef *htim);
HAL_StatusTypeDef HAL_TIM_Base_Stop(TIM_HandleTypeDef *htim);
HAL_StatusTypeDef HAL_TIM_Base_Start_IT(TIM_HandleTypeDef *htim);
HAL_StatusTypeDef HAL_TIM_Base_Stop_IT(TIM_HandleTypeDef *htim);
HAL_StatusTypeDef HAL_TIM_Base_Start_DMA(TIM_HandleTypeDef *htim, const uint32_t *pData, uint16_t Length);
HAL_StatusTypeDef HAL_TIM_Base_Stop_DMA(TIM_HandleTypeDef *htim);

/* Comparaison de sortie */
HAL_StatusTypeDef HAL_TIM_OC_Init(TIM_HandleTypeDef *htim);
HAL_StatusTypeDef HAL_TIM_OC_DeInit(TIM_HandleTypeDef *htim);
void HAL_TIM_OC_MspInit(TIM_HandleTypeDef *htim);
void HAL_TIM_OC_MspDeInit(TIM_HandleTypeDef *htim);
HAL_StatusTypeDef HAL_TIM_OC_Start(TIM_HandleTypeDef *htim, uint32_t Channel);
HAL_StatusTypeDef HAL_TIM_OC_Stop(TIM_HandleTypeDef *htim, uint32_t Channel);
HAL_StatusTypeDef HAL_TIM_OC_Start_IT(TIM_HandleTypeDef *htim, uint32_t Channel);
HAL_StatusTypeDef HAL_TIM_OC_Stop_IT(TIM_HandleTypeDef *htim, uint32_t Channel);
HAL_StatusTypeDef HAL_TIM_OC_Start_DMA(TIM_HandleTypeDef *htim, uint32_t Channel, const uint32_t *pData, uint16_t Length);
HAL_StatusTypeDef HAL_TIM_OC_Stop_DMA(TIM_HandleTypeDef *htim, uint32_t Channel);
HAL_StatusTypeDef HAL_TIM_OC_ConfigChannel(TIM_HandleTypeDef *htim, const TIM_OC_InitTypeDef *sConfig, uint32_t Channel);

/* PWM */
HAL_StatusTypeDef HAL_TIM_PWM_Init(TIM_HandleTypeDef *htim);
HAL_StatusTypeDef HAL_TIM_PWM_DeInit(TIM_HandleTypeDef *htim);
void HAL_TIM_PWM_MspInit(TIM_HandleTypeDef *htim);
void HAL_TIM_PWM_MspDeInit(TIM_HandleTypeDef *htim);
HAL_StatusTypeDef HAL_TIM_PWM_Start(TIM_HandleTypeDef *htim, uint32_t Channel);
HAL_StatusTypeDef HAL_TIM_PWM_Stop(TIM_HandleTypeDef *htim, uint32_t Channel);
HAL_StatusTypeDef HAL_TIM_PWM_Start_IT(TIM_HandleTypeDef *htim, uint32_t Channel);
HAL_StatusTypeDef HAL_TIM_PWM_Stop_IT(TIM_HandleTypeDef *htim, uint32_t Channel);
HAL_StatusTypeDef HAL_TIM_PWM_Start_DMA(TIM_HandleTypeDef *htim, uint32_t Channel, const uint32_t *pData, uint16_t Length);
HAL_StatusTypeDef HAL_TIM_PWM_Stop_DMA(TIM_HandleTypeDef *htim, uint32_t Channel);
HAL_StatusTypeDef HAL_TIM_PWM_ConfigChannel(TIM_HandleTypeDef *htim, const TIM_OC_InitTypeDef *sConfig, uint32_t Channel);

/* Capture d'entrée */
HAL_StatusTypeDef HAL_TIM_IC_Init(TIM_HandleTypeDef *htim);
HAL_StatusTypeDef HAL_TIM_IC_DeInit(TIM_HandleTypeDef *htim);
void HAL_TIM_IC_MspInit(TIM_HandleTypeDef *htim);
void HAL_TIM_IC_MspDeInit(TIM_HandleTypeDef *htim);
HAL_StatusTypeDef HAL_TIM_IC_Start(TIM_HandleTypeDef *htim, uint32_t Channel);
HAL_StatusTypeDef HAL_TIM_IC_Stop(TIM_HandleTypeDef *htim, uint32_t Channel);
HAL_StatusTypeDef HAL_TIM_IC_Start_IT(TIM_HandleTypeDef *htim, uint32_t Channel);
HAL_StatusTypeDef HAL_TIM_IC_Stop_IT(TIM_HandleTypeDef *htim, uint32_t Channel);
HAL_StatusTypeDef HAL_TIM_IC_Start_DMA(TIM_HandleTypeDef *htim, uint32_t Channel, uint32_t *pData, uint16_t Length);
HAL_StatusTypeDef HAL_TIM_IC_Stop_DMA(TIM_HandleTypeDef *htim, uint32_t Channel);
HAL_StatusTypeDef HAL_TIM_IC_ConfigChannel(TIM_HandleTypeDef *htim, const TIM_IC_InitTypeDef *sConfig, uint32_t Channel);

/* Impulsion unique */
HAL_StatusTypeDef HAL_TIM_OnePulse_Init(TIM_HandleTypeDef *htim, uint32_t OnePulseMode);
HAL_StatusTypeDef HAL_TIM_OnePulse_ConfigChannel(TIM_HandleTypeDef *htim, TIM_OnePulse_InitTypeDef *sConfig,
                                                 uint32_t OutputChannel, uint32_t InputChannel);
#define TIM_OPMODE_SINGLE     0x00000008U
#define TIM_OPMODE_REPETITIVE 0x00000000U

/* Codeur */
HAL_StatusTypeDef HAL_TIM_Encoder_Init(TIM_HandleTypeDef *htim, const TIM_Encoder_InitTypeDef *sConfig);
HAL_StatusTypeDef HAL_TIM_Encoder_DeInit(TIM_HandleTypeDef *htim);
void HAL_TIM_Encoder_MspInit(TIM_HandleTypeDef *htim);
void HAL_TIM_Encoder_MspDeInit(TIM_HandleTypeDef *htim);
HAL_StatusTypeDef HAL_TIM_Encoder_Start(TIM_HandleTypeDef *htim, uint32_t Channel);
HAL_StatusTypeDef HAL_TIM_Encoder_Stop(TIM_HandleTypeDef *htim, uint32_t Channel);
HAL_StatusTypeDef HAL_TIM_Encoder_Start_IT(TIM_HandleTypeDef *htim, uint32_t Channel);
HAL_StatusTypeDef HAL_TIM_Encoder_Stop_IT(TIM_HandleTypeDef *htim, uint32_t Channel);

/* Configuration, interruptions */
HAL_StatusTypeDef HAL_TIM_ConfigClockSource(TIM_HandleTypeDef *htim, const TIM_ClockConfigTypeDef *sClockSourceConfig);
HAL_StatusTypeDef HAL_TIM_SlaveConfigSynchro(TIM_HandleTypeDef *htim, const TIM_SlaveConfigTypeDef *sSlaveConfig);
HAL_StatusTypeDef HAL_TIM_SlaveConfigSynchro_IT(TIM_HandleTypeDef *htim, const TIM_SlaveConfigTypeDef *sSlaveConfig);
HAL_StatusTypeDef HAL_TIM_GenerateEvent(TIM_HandleTypeDef *htim, uint32_t EventSource);
uint32_t HAL_TIM_ReadCapturedValue(const TIM_HandleTypeDef *htim, uint32_t Channel);
void HAL_TIM_IRQHandler(TIM_HandleTypeDef *htim);

void HAL_TIM_PeriodElapsedCallback(TIM_HandleTypeDef *htim);
void HAL_TIM_PeriodElapsedHalfCpltCallback(TIM_HandleTypeDef *htim);
void HAL_TIM_OC_DelayElapsedCallback(TIM_HandleTypeDef *htim);
void HAL_TIM_IC_CaptureCallback(TIM_HandleTypeDef *htim);
void HAL_TIM_IC_CaptureHalfCpltCallback(TIM_HandleTypeDef *htim);
void HAL_TIM_PWM_PulseFinishedCallback(TIM_HandleTypeDef *htim);
void HAL_TIM_PWM_PulseFinishedHalfCpltCallback(TIM_HandleTypeDef *htim);
void HAL_TIM_TriggerCallback(TIM_HandleTypeDef *htim);
void HAL_TIM_TriggerHalfCpltCallback(TIM_HandleTypeDef *htim);
void HAL_TIM_ErrorCallback(TIM_HandleTypeDef *htim);

HAL_TIM_StateTypeDef HAL_TIM_Base_GetState(const TIM_HandleTypeDef *htim);
HAL_TIM_StateTypeDef HAL_TIM_OC_GetState(const TIM_HandleTypeDef *htim);
HAL_TIM_StateTypeDef HAL_TIM_PWM_GetState(const TIM_HandleTypeDef *htim);
HAL_TIM_StateTypeDef HAL_TIM_IC_GetState(const TIM_HandleTypeDef *htim);
HAL_TIM_StateTypeDef HAL_TIM_Encoder_GetState(const TIM_HandleTypeDef *htim);
HAL_TIM_ActiveChannel HAL_TIM_GetActiveChannel(const TIM_HandleTypeDef *htim);

/* Extensions (TIM1 avancé, synchronisation) */
HAL_StatusTypeDef HAL_TIMEx_MasterConfigSynchronization(TIM_HandleTypeDef *htim, const TIM_MasterConfigTypeDef *sMasterConfig);
HAL_StatusTypeDef HAL_TIMEx_ConfigBreakDeadTime(TIM_HandleTypeDef *htim, const TIM_BreakDeadTimeConfigTypeDef *sBreakDeadTimeConfig);
HAL_StatusTypeDef HAL_TIMEx_RemapConfig(TIM_HandleTypeDef *htim, uint32_t Remap);
HAL_StatusTypeDef HAL_TIMEx_PWMN_Start(TIM_HandleTypeDef *htim, uint32_t Channel);
HAL_StatusTypeDef HAL_TIMEx_PWMN_Stop(TIM_HandleTypeDef *htim, uint32_t Channel);
HAL_StatusTypeDef HAL_TIMEx_PWMN_Start_IT(TIM_HandleTypeDef *htim, uint32_t Channel);
HAL_StatusTypeDef HAL_TIMEx_PWMN_Stop_IT(TIM_HandleTypeDef *htim, uint32_t Channel);
HAL_StatusTypeDef HAL_TIMEx_OCN_Start(TIM_HandleTypeDef *htim, uint32_t Channel);
HAL_StatusTypeDef HAL_TIMEx_OCN_Stop(TIM_HandleTypeDef *htim, uint32_t Channel);
void HAL_TIMEx_CommutCallback(TIM_HandleTypeDef *htim);
void HAL_TIMEx_BreakCallback(TIM_HandleTypeDef *htim);

#ifdef __cplusplus
}
#endif

#endif /* PREBENCH_STM32F4XX_HAL_TIM_H */
