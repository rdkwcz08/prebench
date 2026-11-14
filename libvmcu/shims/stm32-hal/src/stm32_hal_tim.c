/*
 * libvmcu/shims/stm32-hal/src/stm32_hal_tim.c — Module TIM : PWM simulée (phase 2).
 *
 * On simule le rapport cyclique, pas le signal (section 8.2). Quand un canal
 * PWM tourne (HAL_TIM_PWM_Start), sa fréquence et son rapport cyclique sont
 * calculés comme sur la puce, puis passent par vmcu_pwm_* :
 *   fréquence = horloge du timer / ((PSC + 1) × (ARR + 1)), divisée par 2 en
 *               mode centré ;
 *   rapport   = min(CCRx, ARR + 1) / (ARR + 1), inversé en PWM2 ou avec la
 *               polarité basse.
 * Horloge des timers : déduite de la configuration RCC du firmware
 * (stm32_hal.c), sans simuler l'horloge : APB2 pour TIM1 et TIM9 à TIM11,
 * APB1 pour les autres, doublée quand le bus est divisé (84 MHz pour tous
 * avec la configuration CubeMX de la Nucleo-F401RE ; 16 MHz sans
 * SystemClock_Config, comme la puce au démarrage).
 * __HAL_TIM_SET_COMPARE, __HAL_TIM_SET_AUTORELOAD et __HAL_TIM_SET_PRESCALER
 * mettent à jour un canal en marche, sans fronts ni messages superflus.
 *
 * Non simulés (compilent et avertissent) : compteur, interruptions et
 * callbacks des timers, capture d'entrée, codeur, sorties complémentaires,
 * variantes DMA. Les *_Init appellent leur MspInit si le handle est neuf.
 */
#include "stm32_hal_internal.h"

#if defined(__GNUC__) || defined(__clang__)
#pragma GCC diagnostic ignored "-Wunused-parameter"
#endif

#define STUB_OK() do { PREBENCH_UNSIMULATED(); return HAL_OK; } while (0)

#define TIM_COUNT 12

typedef void (*tim_msp_fn)(TIM_HandleTypeDef *htim);

/* Canal de sortie d'un timer : configuration et marche. */
typedef struct {
    uint32_t mode;     /* TIM_OCMODE_PWM1 ou PWM2 */
    uint32_t polarity; /* TIM_OCPOLARITY_HIGH ou LOW */
    bool     running;
} tim_channel_t;

static tim_channel_t tim_channels[TIM_COUNT][4];
static uint32_t tim_counter_mode[TIM_COUNT];

/* Numéro du timer (TIM2 -> 2), ou -1 pour une instance inconnue. */
static int tim_index(const TIM_TypeDef *tim)
{
    if (tim < &prebench_tim[0] || tim >= &prebench_tim[TIM_COUNT]) {
        return -1;
    }
    return (int)(tim - &prebench_tim[0]);
}

/* TIM_CHANNEL_1..4 -> 0..3, ou -1. */
static int channel_index(uint32_t channel)
{
    if (channel > TIM_CHANNEL_4 || (channel % 4U) != 0U) {
        return -1;
    }
    return (int)(channel / 4U);
}

/* Fréquence et rapport cyclique d'un canal en marche, envoyés à libvmcu. */
static void pwm_apply(const TIM_TypeDef *tim, int ch)
{
    int t = tim_index(tim);
    if (t < 0 || ch < 0 || !tim_channels[t][ch].running) {
        return;
    }
    const tim_channel_t *c = &tim_channels[t][ch];
    uint64_t period = (uint64_t)tim->ARR + 1u;
    uint64_t ticks = ((uint64_t)tim->PSC + 1u) * period;
    bool apb2 = (t == 1 || t >= 9);
    uint64_t freq = ((uint64_t)prebench_tim_clock_hz(apb2) + ticks / 2u) / ticks;
    if ((tim_counter_mode[t] & TIM_COUNTERMODE_CENTERALIGNED3) != 0U) {
        freq /= 2u; /* comptage aller-retour : période doublée */
    }
    if (freq == 0u) {
        freq = 1u; /* 0 signifierait « arrêt » (vmcu_pwm_config) */
    }
    const __IO uint32_t *regs[4] = { &tim->CCR1, &tim->CCR2, &tim->CCR3, &tim->CCR4 };
    uint64_t compare = *regs[ch];
    if (compare > period) {
        compare = period;
    }
    uint64_t duty = (compare * VMCU_PWM_DUTY_MAX + period / 2u) / period;
    bool inverted = (c->mode == TIM_OCMODE_PWM2) != (c->polarity == TIM_OCPOLARITY_LOW);
    if (inverted) {
        duty = VMCU_PWM_DUTY_MAX - duty;
    }
    /* Rapport cyclique d'abord : au démarrage, le cœur ne voit jamais une
     * fréquence avec un rapport encore inconnu (impulsion nulle). */
    (void)vmcu_pwm_set_duty((uint8_t)t, (uint8_t)(ch + 1), (uint16_t)duty);
    (void)vmcu_pwm_config((uint8_t)t, (uint8_t)(ch + 1), (uint32_t)(freq > UINT32_MAX ? UINT32_MAX : freq));
}

/* Tous les canaux en marche d'un timer (ARR ou PSC modifié). */
static void pwm_apply_all(const TIM_TypeDef *tim)
{
    for (int ch = 0; ch < 4; ++ch) {
        pwm_apply(tim, ch);
    }
}

static HAL_StatusTypeDef pwm_start(TIM_HandleTypeDef *htim, uint32_t channel)
{
    int t = tim_index(htim->Instance);
    int ch = channel_index(channel);
    if (t < 0 || ch < 0) {
        return HAL_ERROR;
    }
    tim_channels[t][ch].running = true;
    tim_counter_mode[t] = htim->Init.CounterMode;
    htim->ChannelState[ch] = HAL_TIM_CHANNEL_STATE_BUSY;
    htim->State = HAL_TIM_STATE_BUSY;
    pwm_apply(htim->Instance, ch);
    return HAL_OK;
}

static HAL_StatusTypeDef pwm_stop(TIM_HandleTypeDef *htim, uint32_t channel)
{
    int t = tim_index(htim->Instance);
    int ch = channel_index(channel);
    if (t < 0 || ch < 0) {
        return HAL_ERROR;
    }
    if (tim_channels[t][ch].running) {
        tim_channels[t][ch].running = false;
        (void)vmcu_pwm_config((uint8_t)t, (uint8_t)(ch + 1), 0); /* sortie arrêtée */
    }
    htim->ChannelState[ch] = HAL_TIM_CHANNEL_STATE_READY;
    htim->State = HAL_TIM_STATE_READY;
    return HAL_OK;
}

static HAL_StatusTypeDef pwm_config_channel(TIM_HandleTypeDef *htim, const TIM_OC_InitTypeDef *sConfig,
                                            uint32_t channel)
{
    int t = tim_index(htim->Instance);
    int ch = channel_index(channel);
    if (t < 0 || ch < 0 || sConfig == NULL) {
        return HAL_ERROR;
    }
    __IO uint32_t *regs[4] = { &htim->Instance->CCR1, &htim->Instance->CCR2, &htim->Instance->CCR3,
                               &htim->Instance->CCR4 };
    *regs[ch] = sConfig->Pulse;
    tim_channels[t][ch].mode = sConfig->OCMode;
    tim_channels[t][ch].polarity = sConfig->OCPolarity;
    if (sConfig->OCMode != TIM_OCMODE_PWM1 && sConfig->OCMode != TIM_OCMODE_PWM2) {
        vmcu_warn_unsimulated("TIM : mode de comparaison autre que PWM1 ou PWM2");
    }
    pwm_apply(htim->Instance, ch);
    return HAL_OK;
}

static HAL_StatusTypeDef tim_init(TIM_HandleTypeDef *htim, tim_msp_fn msp, const char *name)
{
    if (htim == NULL || htim->Instance == NULL) {
        return HAL_ERROR;
    }
    if (name != NULL) {
        vmcu_warn_unsimulated(name);
    }
    if (htim->State == HAL_TIM_STATE_RESET) {
        htim->Lock = HAL_UNLOCKED;
        if (msp != NULL) {
            msp(htim);
        }
    }
    htim->Instance->ARR = htim->Init.Period;
    htim->Instance->PSC = htim->Init.Prescaler;
    int t = tim_index(htim->Instance);
    if (t >= 0) {
        tim_counter_mode[t] = htim->Init.CounterMode;
    }
    for (int i = 0; i < 4; ++i) {
        htim->ChannelState[i] = HAL_TIM_CHANNEL_STATE_READY;
        htim->ChannelNState[i] = HAL_TIM_CHANNEL_STATE_READY;
    }
    htim->State = HAL_TIM_STATE_READY;
    pwm_apply_all(htim->Instance);
    return HAL_OK;
}

static HAL_StatusTypeDef tim_deinit(TIM_HandleTypeDef *htim, tim_msp_fn msp)
{
    if (htim == NULL) {
        return HAL_ERROR;
    }
    for (uint32_t ch = 0; ch < 4U; ++ch) {
        (void)pwm_stop(htim, ch * 4U);
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
    __IO uint32_t *reg = ccr(htim->Instance, channel);
    if (reg != NULL) {
        *reg = compare;
        pwm_apply(htim->Instance, channel_index(channel));
    }
}

uint32_t prebench_tim_get_compare(const TIM_HandleTypeDef *htim, uint32_t channel)
{
    __IO uint32_t *reg = ccr(htim->Instance, channel);
    return (reg != NULL) ? *reg : 0U;
}

void prebench_tim_set_autoreload(TIM_HandleTypeDef *htim, uint32_t autoreload)
{
    htim->Instance->ARR = autoreload;
    htim->Init.Period = autoreload;
    pwm_apply_all(htim->Instance);
}

void prebench_tim_set_prescaler(TIM_HandleTypeDef *htim, uint32_t prescaler)
{
    htim->Instance->PSC = prescaler;
    pwm_apply_all(htim->Instance);
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

HAL_StatusTypeDef HAL_TIM_Base_Init(TIM_HandleTypeDef *htim) { return tim_init(htim, HAL_TIM_Base_MspInit, NULL); }
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

/* --- PWM ------------------------------------------------------------------------------------ */

HAL_StatusTypeDef HAL_TIM_PWM_Init(TIM_HandleTypeDef *htim) { return tim_init(htim, HAL_TIM_PWM_MspInit, NULL); }
HAL_StatusTypeDef HAL_TIM_PWM_DeInit(TIM_HandleTypeDef *htim) { return tim_deinit(htim, HAL_TIM_PWM_MspDeInit); }
HAL_StatusTypeDef HAL_TIM_PWM_Start(TIM_HandleTypeDef *htim, uint32_t Channel) { return pwm_start(htim, Channel); }
HAL_StatusTypeDef HAL_TIM_PWM_Stop(TIM_HandleTypeDef *htim, uint32_t Channel) { return pwm_stop(htim, Channel); }
HAL_StatusTypeDef HAL_TIM_PWM_Start_IT(TIM_HandleTypeDef *htim, uint32_t Channel)
{
    /* La sortie est simulée ; l'interruption de fin d'impulsion ne l'est pas. */
    vmcu_warn_unsimulated("HAL_TIM_PWM_PulseFinishedCallback");
    return pwm_start(htim, Channel);
}
HAL_StatusTypeDef HAL_TIM_PWM_Stop_IT(TIM_HandleTypeDef *htim, uint32_t Channel) { return pwm_stop(htim, Channel); }
HAL_StatusTypeDef HAL_TIM_PWM_Start_DMA(TIM_HandleTypeDef *htim, uint32_t Channel, const uint32_t *pData, uint16_t Length) { STUB_OK(); }
HAL_StatusTypeDef HAL_TIM_PWM_Stop_DMA(TIM_HandleTypeDef *htim, uint32_t Channel) { return pwm_stop(htim, Channel); }
HAL_StatusTypeDef HAL_TIM_PWM_ConfigChannel(TIM_HandleTypeDef *htim, const TIM_OC_InitTypeDef *sConfig, uint32_t Channel)
{
    return pwm_config_channel(htim, sConfig, Channel);
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
