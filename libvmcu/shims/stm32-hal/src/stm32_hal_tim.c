/*
 * libvmcu/shims/stm32-hal/src/stm32_hal_tim.c — Module TIM, non simulé en phase 1.
 *
 * Les *_Init appellent leur MspInit si le handle est neuf ; les registres CCRx,
 * ARR, PSC et CNT servent de stockage aux macros __HAL_TIM_*. Aucun compteur
 * ne tourne et aucun callback n'est appelé. La PWM arrive en phase 2 : elle
 * passera par vmcu_pwm_* depuis prebench_tim_set_compare et HAL_TIM_PWM_Start.
 */
#include "stm32_hal_internal.h"

#if defined(__GNUC__) || defined(__clang__)
#pragma GCC diagnostic ignored "-Wunused-parameter"
#endif

#define STUB_OK() do { PREBENCH_UNSIMULATED(); return HAL_OK; } while (0)

typedef void (*tim_msp_fn)(TIM_HandleTypeDef *htim);

static HAL_StatusTypeDef tim_init(TIM_HandleTypeDef *htim, tim_msp_fn msp, const char *name)
{
    if (htim == NULL || htim->Instance == NULL) {
        return HAL_ERROR;
    }
    vmcu_warn_unsimulated(name);
    if (htim->State == HAL_TIM_STATE_RESET) {
        htim->Lock = HAL_UNLOCKED;
        if (msp != NULL) {
            msp(htim);
        }
    }
    htim->Instance->ARR = htim->Init.Period;
    htim->Instance->PSC = htim->Init.Prescaler;
    for (int i = 0; i < 4; ++i) {
        htim->ChannelState[i] = HAL_TIM_CHANNEL_STATE_READY;
        htim->ChannelNState[i] = HAL_TIM_CHANNEL_STATE_READY;
    }
    htim->State = HAL_TIM_STATE_READY;
    return HAL_OK;
}

static HAL_StatusTypeDef tim_deinit(TIM_HandleTypeDef *htim, tim_msp_fn msp)
{
    if (htim == NULL) {
        return HAL_ERROR;
    }
    msp(htim);
    htim->State = HAL_TIM_STATE_RESET;
    return HAL_OK;
}

/* --- Macros __HAL_TIM_* ----------------------------------------------------------------- */

static __IO uint32_t *ccr(TIM_TypeDef *tim, uint32_t channel)
{
    switch (channel) {
    case TIM_CHANNEL_1: return &tim->CCR1;
    case TIM_CHANNEL_2: return &tim->CCR2;
    case TIM_CHANNEL_3: return &tim->CCR3;
    case TIM_CHANNEL_4: return &tim->CCR4;
    default:            return NULL;
    }
}

void prebench_tim_set_compare(TIM_HandleTypeDef *htim, uint32_t channel, uint32_t compare)
{
    vmcu_warn_unsimulated("__HAL_TIM_SET_COMPARE");
    __IO uint32_t *reg = ccr(htim->Instance, channel);
    if (reg != NULL) {
        *reg = compare;
    }
}

uint32_t prebench_tim_get_compare(const TIM_HandleTypeDef *htim, uint32_t channel)
{
    __IO uint32_t *reg = ccr(htim->Instance, channel);
    return (reg != NULL) ? *reg : 0U;
}

void prebench_tim_set_autoreload(TIM_HandleTypeDef *htim, uint32_t autoreload)
{
    vmcu_warn_unsimulated("__HAL_TIM_SET_AUTORELOAD");
    htim->Instance->ARR = autoreload;
    htim->Init.Period = autoreload;
}

void prebench_tim_set_prescaler(TIM_HandleTypeDef *htim, uint32_t prescaler)
{
    vmcu_warn_unsimulated("__HAL_TIM_SET_PRESCALER");
    htim->Instance->PSC = prescaler;
}

void prebench_tim_set_counter(TIM_HandleTypeDef *htim, uint32_t counter)
{
    htim->Instance->CNT = counter;
}

uint32_t prebench_tim_get_counter(const TIM_HandleTypeDef *htim)
{
    vmcu_warn_unsimulated("__HAL_TIM_GET_COUNTER");
    return htim->Instance->CNT;
}

/* --- Base --------------------------------------------------------------------------------- */

HAL_StatusTypeDef HAL_TIM_Base_Init(TIM_HandleTypeDef *htim) { return tim_init(htim, HAL_TIM_Base_MspInit, "HAL_TIM_Base_Init"); }
HAL_StatusTypeDef HAL_TIM_Base_DeInit(TIM_HandleTypeDef *htim) { return tim_deinit(htim, HAL_TIM_Base_MspDeInit); }
HAL_StatusTypeDef HAL_TIM_Base_Start(TIM_HandleTypeDef *htim) { STUB_OK(); }
HAL_StatusTypeDef HAL_TIM_Base_Stop(TIM_HandleTypeDef *htim) { return HAL_OK; }
HAL_StatusTypeDef HAL_TIM_Base_Start_IT(TIM_HandleTypeDef *htim) { STUB_OK(); }
HAL_StatusTypeDef HAL_TIM_Base_Stop_IT(TIM_HandleTypeDef *htim) { return HAL_OK; }
HAL_StatusTypeDef HAL_TIM_Base_Start_DMA(TIM_HandleTypeDef *htim, const uint32_t *pData, uint16_t Length) { STUB_OK(); }
HAL_StatusTypeDef HAL_TIM_Base_Stop_DMA(TIM_HandleTypeDef *htim) { return HAL_OK; }

/* --- Comparaison de sortie ---------------------------------------------------------------- */

HAL_StatusTypeDef HAL_TIM_OC_Init(TIM_HandleTypeDef *htim) { return tim_init(htim, HAL_TIM_OC_MspInit, "HAL_TIM_OC_Init"); }
HAL_StatusTypeDef HAL_TIM_OC_DeInit(TIM_HandleTypeDef *htim) { return tim_deinit(htim, HAL_TIM_OC_MspDeInit); }
HAL_StatusTypeDef HAL_TIM_OC_Start(TIM_HandleTypeDef *htim, uint32_t Channel) { STUB_OK(); }
HAL_StatusTypeDef HAL_TIM_OC_Stop(TIM_HandleTypeDef *htim, uint32_t Channel) { return HAL_OK; }
HAL_StatusTypeDef HAL_TIM_OC_Start_IT(TIM_HandleTypeDef *htim, uint32_t Channel) { STUB_OK(); }
HAL_StatusTypeDef HAL_TIM_OC_Stop_IT(TIM_HandleTypeDef *htim, uint32_t Channel) { return HAL_OK; }
HAL_StatusTypeDef HAL_TIM_OC_Start_DMA(TIM_HandleTypeDef *htim, uint32_t Channel, const uint32_t *pData, uint16_t Length) { STUB_OK(); }
HAL_StatusTypeDef HAL_TIM_OC_Stop_DMA(TIM_HandleTypeDef *htim, uint32_t Channel) { return HAL_OK; }
HAL_StatusTypeDef HAL_TIM_OC_ConfigChannel(TIM_HandleTypeDef *htim, const TIM_OC_InitTypeDef *sConfig, uint32_t Channel)
{
    __IO uint32_t *reg = ccr(htim->Instance, Channel);
    if (reg != NULL && sConfig != NULL) {
        *reg = sConfig->Pulse;
    }
    STUB_OK();
}

/* --- PWM (simulation en phase 2) ----------------------------------------------------------- */

HAL_StatusTypeDef HAL_TIM_PWM_Init(TIM_HandleTypeDef *htim) { return tim_init(htim, HAL_TIM_PWM_MspInit, "HAL_TIM_PWM_Init"); }
HAL_StatusTypeDef HAL_TIM_PWM_DeInit(TIM_HandleTypeDef *htim) { return tim_deinit(htim, HAL_TIM_PWM_MspDeInit); }
HAL_StatusTypeDef HAL_TIM_PWM_Start(TIM_HandleTypeDef *htim, uint32_t Channel) { STUB_OK(); }
HAL_StatusTypeDef HAL_TIM_PWM_Stop(TIM_HandleTypeDef *htim, uint32_t Channel) { return HAL_OK; }
HAL_StatusTypeDef HAL_TIM_PWM_Start_IT(TIM_HandleTypeDef *htim, uint32_t Channel) { STUB_OK(); }
HAL_StatusTypeDef HAL_TIM_PWM_Stop_IT(TIM_HandleTypeDef *htim, uint32_t Channel) { return HAL_OK; }
HAL_StatusTypeDef HAL_TIM_PWM_Start_DMA(TIM_HandleTypeDef *htim, uint32_t Channel, const uint32_t *pData, uint16_t Length) { STUB_OK(); }
HAL_StatusTypeDef HAL_TIM_PWM_Stop_DMA(TIM_HandleTypeDef *htim, uint32_t Channel) { return HAL_OK; }
HAL_StatusTypeDef HAL_TIM_PWM_ConfigChannel(TIM_HandleTypeDef *htim, const TIM_OC_InitTypeDef *sConfig, uint32_t Channel)
{
    __IO uint32_t *reg = ccr(htim->Instance, Channel);
    if (reg != NULL && sConfig != NULL) {
        *reg = sConfig->Pulse;
    }
    STUB_OK();
}

/* --- Capture d'entrée ----------------------------------------------------------------------- */

HAL_StatusTypeDef HAL_TIM_IC_Init(TIM_HandleTypeDef *htim) { return tim_init(htim, HAL_TIM_IC_MspInit, "HAL_TIM_IC_Init"); }
HAL_StatusTypeDef HAL_TIM_IC_DeInit(TIM_HandleTypeDef *htim) { return tim_deinit(htim, HAL_TIM_IC_MspDeInit); }
HAL_StatusTypeDef HAL_TIM_IC_Start(TIM_HandleTypeDef *htim, uint32_t Channel) { STUB_OK(); }
HAL_StatusTypeDef HAL_TIM_IC_Stop(TIM_HandleTypeDef *htim, uint32_t Channel) { return HAL_OK; }
HAL_StatusTypeDef HAL_TIM_IC_Start_IT(TIM_HandleTypeDef *htim, uint32_t Channel) { STUB_OK(); }
HAL_StatusTypeDef HAL_TIM_IC_Stop_IT(TIM_HandleTypeDef *htim, uint32_t Channel) { return HAL_OK; }
HAL_StatusTypeDef HAL_TIM_IC_Start_DMA(TIM_HandleTypeDef *htim, uint32_t Channel, uint32_t *pData, uint16_t Length) { STUB_OK(); }
HAL_StatusTypeDef HAL_TIM_IC_Stop_DMA(TIM_HandleTypeDef *htim, uint32_t Channel) { return HAL_OK; }
HAL_StatusTypeDef HAL_TIM_IC_ConfigChannel(TIM_HandleTypeDef *htim, const TIM_IC_InitTypeDef *sConfig, uint32_t Channel) { STUB_OK(); }

/* --- Impulsion unique, codeur ------------------------------------------------------------------ */

HAL_StatusTypeDef HAL_TIM_OnePulse_Init(TIM_HandleTypeDef *htim, uint32_t OnePulseMode) { return tim_init(htim, NULL, "HAL_TIM_OnePulse_Init"); }
HAL_StatusTypeDef HAL_TIM_OnePulse_ConfigChannel(TIM_HandleTypeDef *htim, TIM_OnePulse_InitTypeDef *sConfig,
                                                 uint32_t OutputChannel, uint32_t InputChannel) { STUB_OK(); }

HAL_StatusTypeDef HAL_TIM_Encoder_Init(TIM_HandleTypeDef *htim, const TIM_Encoder_InitTypeDef *sConfig) { return tim_init(htim, HAL_TIM_Encoder_MspInit, "HAL_TIM_Encoder_Init"); }
HAL_StatusTypeDef HAL_TIM_Encoder_DeInit(TIM_HandleTypeDef *htim) { return tim_deinit(htim, HAL_TIM_Encoder_MspDeInit); }
HAL_StatusTypeDef HAL_TIM_Encoder_Start(TIM_HandleTypeDef *htim, uint32_t Channel) { STUB_OK(); }
HAL_StatusTypeDef HAL_TIM_Encoder_Stop(TIM_HandleTypeDef *htim, uint32_t Channel) { return HAL_OK; }
HAL_StatusTypeDef HAL_TIM_Encoder_Start_IT(TIM_HandleTypeDef *htim, uint32_t Channel) { STUB_OK(); }
HAL_StatusTypeDef HAL_TIM_Encoder_Stop_IT(TIM_HandleTypeDef *htim, uint32_t Channel) { return HAL_OK; }

/* --- Configuration, interruptions, états -------------------------------------------------------- */

HAL_StatusTypeDef HAL_TIM_ConfigClockSource(TIM_HandleTypeDef *htim, const TIM_ClockConfigTypeDef *sClockSourceConfig) { return HAL_OK; }
HAL_StatusTypeDef HAL_TIM_SlaveConfigSynchro(TIM_HandleTypeDef *htim, const TIM_SlaveConfigTypeDef *sSlaveConfig) { STUB_OK(); }
HAL_StatusTypeDef HAL_TIM_SlaveConfigSynchro_IT(TIM_HandleTypeDef *htim, const TIM_SlaveConfigTypeDef *sSlaveConfig) { STUB_OK(); }
HAL_StatusTypeDef HAL_TIM_GenerateEvent(TIM_HandleTypeDef *htim, uint32_t EventSource) { STUB_OK(); }
uint32_t HAL_TIM_ReadCapturedValue(const TIM_HandleTypeDef *htim, uint32_t Channel)
{
    PREBENCH_UNSIMULATED();
    return 0U;
}
void HAL_TIM_IRQHandler(TIM_HandleTypeDef *htim) {}

HAL_TIM_StateTypeDef HAL_TIM_Base_GetState(const TIM_HandleTypeDef *htim) { return htim->State; }
HAL_TIM_StateTypeDef HAL_TIM_OC_GetState(const TIM_HandleTypeDef *htim) { return htim->State; }
HAL_TIM_StateTypeDef HAL_TIM_PWM_GetState(const TIM_HandleTypeDef *htim) { return htim->State; }
HAL_TIM_StateTypeDef HAL_TIM_IC_GetState(const TIM_HandleTypeDef *htim) { return htim->State; }
HAL_TIM_StateTypeDef HAL_TIM_Encoder_GetState(const TIM_HandleTypeDef *htim) { return htim->State; }
HAL_TIM_ActiveChannel HAL_TIM_GetActiveChannel(const TIM_HandleTypeDef *htim) { return htim->Channel; }

/* --- Extensions -------------------------------------------------------------------------------- */

HAL_StatusTypeDef HAL_TIMEx_MasterConfigSynchronization(TIM_HandleTypeDef *htim, const TIM_MasterConfigTypeDef *sMasterConfig) { return HAL_OK; }
HAL_StatusTypeDef HAL_TIMEx_ConfigBreakDeadTime(TIM_HandleTypeDef *htim, const TIM_BreakDeadTimeConfigTypeDef *sBreakDeadTimeConfig) { return HAL_OK; }
HAL_StatusTypeDef HAL_TIMEx_RemapConfig(TIM_HandleTypeDef *htim, uint32_t Remap) { return HAL_OK; }
HAL_StatusTypeDef HAL_TIMEx_PWMN_Start(TIM_HandleTypeDef *htim, uint32_t Channel) { STUB_OK(); }
HAL_StatusTypeDef HAL_TIMEx_PWMN_Stop(TIM_HandleTypeDef *htim, uint32_t Channel) { return HAL_OK; }
HAL_StatusTypeDef HAL_TIMEx_PWMN_Start_IT(TIM_HandleTypeDef *htim, uint32_t Channel) { STUB_OK(); }
HAL_StatusTypeDef HAL_TIMEx_PWMN_Stop_IT(TIM_HandleTypeDef *htim, uint32_t Channel) { return HAL_OK; }
HAL_StatusTypeDef HAL_TIMEx_OCN_Start(TIM_HandleTypeDef *htim, uint32_t Channel) { STUB_OK(); }
HAL_StatusTypeDef HAL_TIMEx_OCN_Stop(TIM_HandleTypeDef *htim, uint32_t Channel) { return HAL_OK; }
