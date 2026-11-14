/*
 * libvmcu/shims/stm32-hal/src/stm32_hal_adc.c — Module ADC : conversions simulées (phase 2).
 *
 * Une conversion se termine aussitôt (aucun temps d'échantillonnage) et lit la
 * tension de la broche dans le cache de libvmcu (vmcu_adc_read) : potentiomètre
 * ou source analogique câblés sur la paillasse. La valeur brute arrive à la
 * résolution de la carte (adc_bits) ; le shim la ramène à la résolution
 * choisie (ADC_RESOLUTION_12B..6B) et à l'alignement demandé.
 *
 * Séquence régulière : les rangs de HAL_ADC_ConfigChannel sont rangés dans
 * SQR1..SQR3, comme sur la puce ; en mode scan, chaque HAL_ADC_GetValue lit le
 * rang suivant. Start_IT appelle HAL_ADC_ConvCpltCallback une fois, depuis le
 * fil d'interruption de libvmcu.
 *
 * Non simulés (compilent et avertissent) : DMA (HAL_ADC_Start_DMA), capteurs
 * internes (température, VREFINT, VBAT), conversions continues sous
 * interruption (une seule conversion), injection, chien de garde analogique.
 */
#include "stm32_hal_internal.h"

#if defined(__GNUC__) || defined(__clang__)
#pragma GCC diagnostic ignored "-Wunused-parameter"
#endif

#define STUB_OK() do { PREBENCH_UNSIMULATED(); return HAL_OK; } while (0)

#define ADC_RANKS 16U

/* Numéro de l'ADC (ADC1 -> 1), ou 0 pour une instance inconnue. */
static uint8_t adc_unit(const ADC_HandleTypeDef *hadc)
{
    const ADC_TypeDef *adc = hadc->Instance;
    if (adc < &prebench_adc[0] || adc >= &prebench_adc[2]) {
        return 0;
    }
    return (uint8_t)(adc - &prebench_adc[0]);
}

/* Registre de séquence et décalage du rang 1..16 (SQR3 : 1-6, SQR2 : 7-12, SQR1 : 13-16). */
static __IO uint32_t *rank_reg(ADC_TypeDef *adc, uint32_t rank, unsigned *shift)
{
    *shift = 5U * ((rank - 1U) % 6U);
    if (rank <= 6U) {
        return &adc->SQR3;
    }
    return (rank <= 12U) ? &adc->SQR2 : &adc->SQR1;
}

static uint32_t rank_channel(const ADC_HandleTypeDef *hadc, uint32_t rank)
{
    unsigned shift;
    const __IO uint32_t *reg = rank_reg(hadc->Instance, rank, &shift);
    return (*reg >> shift) & 0x1FU;
}

/* Bits de la résolution choisie (ADC_RESOLUTION_12B..6B). */
static unsigned resolution_bits(const ADC_HandleTypeDef *hadc)
{
    switch (hadc->Init.Resolution) {
    case ADC_RESOLUTION_10B: return 10U;
    case ADC_RESOLUTION_8B:  return 8U;
    case ADC_RESOLUTION_6B:  return 6U;
    default:                 return 12U;
    }
}

HAL_StatusTypeDef HAL_ADC_Init(ADC_HandleTypeDef *hadc)
{
    if (hadc == NULL || hadc->Instance == NULL) {
        return HAL_ERROR;
    }
    if (hadc->State == HAL_ADC_STATE_RESET) {
        hadc->Lock = HAL_UNLOCKED;
        HAL_ADC_MspInit(hadc);
    }
    hadc->ErrorCode = HAL_ADC_ERROR_NONE;
    hadc->NbrOfCurrentConversionRank = 0U;
    hadc->State = HAL_ADC_STATE_READY;
    return HAL_OK;
}

HAL_StatusTypeDef HAL_ADC_DeInit(ADC_HandleTypeDef *hadc)
{
    if (hadc == NULL) {
        return HAL_ERROR;
    }
    HAL_ADC_MspDeInit(hadc);
    hadc->State = HAL_ADC_STATE_RESET;
    return HAL_OK;
}

HAL_StatusTypeDef HAL_ADC_ConfigChannel(ADC_HandleTypeDef *hadc, const ADC_ChannelConfTypeDef *sConfig)
{
    if (hadc == NULL || sConfig == NULL || sConfig->Rank < 1U || sConfig->Rank > ADC_RANKS ||
        sConfig->Channel > ADC_CHANNEL_18) {
        return HAL_ERROR;
    }
    unsigned shift;
    __IO uint32_t *reg = rank_reg(hadc->Instance, sConfig->Rank, &shift);
    *reg = (*reg & ~(0x1FU << shift)) | (sConfig->Channel << shift);
    if (sConfig->Channel >= ADC_CHANNEL_16) {
        vmcu_warn_unsimulated("ADC : capteurs internes (température, VREFINT, VBAT)");
    } else {
        (void)vmcu_adc_config(adc_unit(hadc), (uint8_t)sConfig->Channel);
    }
    return HAL_OK;
}

/* Conversion terminée aussitôt : la séquence repart du rang 1. */
static void start_conversion(ADC_HandleTypeDef *hadc)
{
    hadc->NbrOfCurrentConversionRank = 0U;
    hadc->State = (hadc->State & ~HAL_ADC_STATE_READY) | HAL_ADC_STATE_REG_BUSY | HAL_ADC_STATE_REG_EOC;
}

static void stop_conversion(ADC_HandleTypeDef *hadc)
{
    hadc->State = (hadc->State & ~(HAL_ADC_STATE_REG_BUSY | HAL_ADC_STATE_REG_EOC)) | HAL_ADC_STATE_READY;
}

HAL_StatusTypeDef HAL_ADC_Start(ADC_HandleTypeDef *hadc)
{
    start_conversion(hadc);
    return HAL_OK;
}

HAL_StatusTypeDef HAL_ADC_Stop(ADC_HandleTypeDef *hadc)
{
    stop_conversion(hadc);
    return HAL_OK;
}

HAL_StatusTypeDef HAL_ADC_PollForConversion(ADC_HandleTypeDef *hadc, uint32_t Timeout)
{
    hadc->State |= HAL_ADC_STATE_REG_EOC;
    return HAL_OK;
}

HAL_StatusTypeDef HAL_ADC_PollForEvent(ADC_HandleTypeDef *hadc, uint32_t EventType, uint32_t Timeout)
{
    PREBENCH_UNSIMULATED();
    prebench_stub_wait(Timeout);
    return HAL_TIMEOUT;
}

static void conv_complete(void *arg)
{
    HAL_ADC_ConvCpltCallback((ADC_HandleTypeDef *)arg);
}

HAL_StatusTypeDef HAL_ADC_Start_IT(ADC_HandleTypeDef *hadc)
{
    start_conversion(hadc);
    if (hadc->Init.ContinuousConvMode == ENABLE) {
        vmcu_warn_unsimulated("ADC : conversions continues sous interruption (une seule conversion)");
    }
    return (vmcu_irq_post(conv_complete, hadc) == VMCU_OK) ? HAL_OK : HAL_ERROR;
}

HAL_StatusTypeDef HAL_ADC_Stop_IT(ADC_HandleTypeDef *hadc)
{
    stop_conversion(hadc);
    return HAL_OK;
}

void HAL_ADC_IRQHandler(ADC_HandleTypeDef *hadc) {}

/* DMA hors périmètre (section 3.3) : compile et avertit, aucun transfert. */
HAL_StatusTypeDef HAL_ADC_Start_DMA(ADC_HandleTypeDef *hadc, uint32_t *pData, uint32_t Length) { STUB_OK(); }
HAL_StatusTypeDef HAL_ADC_Stop_DMA(ADC_HandleTypeDef *hadc) { return HAL_OK; }

uint32_t HAL_ADC_GetValue(const ADC_HandleTypeDef *hadc)
{
    ADC_HandleTypeDef *h = (ADC_HandleTypeDef *)(uintptr_t)hadc; /* curseur de séquence */
    uint32_t count = (hadc->Init.ScanConvMode != DISABLE && hadc->Init.NbrOfConversion > 1U)
                         ? hadc->Init.NbrOfConversion
                         : 1U;
    if (count > ADC_RANKS) {
        count = ADC_RANKS;
    }
    uint32_t rank = (h->NbrOfCurrentConversionRank % count) + 1U;
    h->NbrOfCurrentConversionRank++;
    uint32_t channel = rank_channel(hadc, rank);
    if (channel >= ADC_CHANNEL_16) {
        return 0U;
    }
    uint32_t raw = vmcu_adc_read(adc_unit(hadc), (uint8_t)channel);
    unsigned board = vmcu_adc_bits();
    unsigned bits = resolution_bits(hadc);
    if (board > bits) {
        raw >>= board - bits;
    } else if (board < bits) {
        raw <<= bits - board;
    }
    if (hadc->Init.DataAlign == ADC_DATAALIGN_LEFT) {
        raw <<= (bits == 6U) ? 2U : 16U - bits; /* 6 bits : alignés dans l'octet */
    }
    return raw;
}

HAL_StatusTypeDef HAL_ADC_AnalogWDGConfig(ADC_HandleTypeDef *hadc, const ADC_AnalogWDGConfTypeDef *AnalogWDGConfig) { STUB_OK(); }
uint32_t HAL_ADC_GetState(const ADC_HandleTypeDef *hadc) { return hadc->State; }
uint32_t HAL_ADC_GetError(const ADC_HandleTypeDef *hadc) { return hadc->ErrorCode; }
