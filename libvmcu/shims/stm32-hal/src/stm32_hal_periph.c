/*
 * libvmcu/shims/stm32-hal/src/stm32_hal_periph.c — Instances des périphériques et
 * modules DMA, ADC, I2C, SPI pas encore simulés (règle 16.2.10). L'UART est
 * simulée dans stm32_hal_uart.c.
 *
 * Comportement commun :
 * - *_Init : appelle le MspInit du module si le handle est neuf (comme le HAL de
 *   ST : CubeMX y configure les broches), passe à l'état READY, renvoie HAL_OK ;
 * - émissions : acceptées sans effet ;
 * - réceptions bloquantes : attente bornée à 10 ms puis HAL_TIMEOUT ;
 * - I2C : comme un bus sans composant (HAL_ERROR, HAL_I2C_ERROR_AF) ;
 * - SPI : octets reçus à 0xFF ;
 * - variantes _IT / _DMA : acceptées, aucun callback n'est appelé.
 * Chaque fonction avertit une fois « <fonction> non simulé ».
 */
#include "stm32_hal_internal.h"

#include <string.h>

#if defined(__GNUC__) || defined(__clang__)
#pragma GCC diagnostic ignored "-Wunused-parameter"
#endif

USART_TypeDef      prebench_usart[7];
TIM_TypeDef        prebench_tim[12];
ADC_TypeDef        prebench_adc[2];
I2C_TypeDef        prebench_i2c[4];
SPI_TypeDef        prebench_spi[5];
DMA_TypeDef        prebench_dma[3];
DMA_Stream_TypeDef prebench_dma_stream[16];

void prebench_stub_wait(uint32_t timeout_ms)
{
    vmcu_delay_ms(timeout_ms < 10U ? timeout_ms : 10U);
}

#define STUB_OK()   do { PREBENCH_UNSIMULATED(); return HAL_OK; } while (0)

/* --- DMA ------------------------------------------------------------------------------ */

HAL_StatusTypeDef HAL_DMA_Init(DMA_HandleTypeDef *hdma)
{
    if (hdma == NULL) {
        return HAL_ERROR;
    }
    PREBENCH_UNSIMULATED();
    hdma->State = HAL_DMA_STATE_READY;
    hdma->ErrorCode = HAL_DMA_ERROR_NONE;
    return HAL_OK;
}

HAL_StatusTypeDef HAL_DMA_DeInit(DMA_HandleTypeDef *hdma)
{
    if (hdma != NULL) {
        hdma->State = HAL_DMA_STATE_RESET;
    }
    return HAL_OK;
}

HAL_StatusTypeDef HAL_DMA_Start(DMA_HandleTypeDef *hdma, uint32_t SrcAddress, uint32_t DstAddress, uint32_t DataLength) { STUB_OK(); }
HAL_StatusTypeDef HAL_DMA_Start_IT(DMA_HandleTypeDef *hdma, uint32_t SrcAddress, uint32_t DstAddress, uint32_t DataLength) { STUB_OK(); }
HAL_StatusTypeDef HAL_DMA_Abort(DMA_HandleTypeDef *hdma) { return HAL_OK; }
HAL_StatusTypeDef HAL_DMA_Abort_IT(DMA_HandleTypeDef *hdma) { return HAL_OK; }
HAL_StatusTypeDef HAL_DMA_PollForTransfer(DMA_HandleTypeDef *hdma, HAL_DMA_LevelCompleteTypeDef CompleteLevel, uint32_t Timeout)
{
    PREBENCH_UNSIMULATED();
    prebench_stub_wait(Timeout);
    return HAL_TIMEOUT;
}
void HAL_DMA_IRQHandler(DMA_HandleTypeDef *hdma) {}
HAL_StatusTypeDef HAL_DMA_CleanCallbacks(DMA_HandleTypeDef *hdma) { return HAL_OK; }
HAL_StatusTypeDef HAL_DMA_RegisterCallback(DMA_HandleTypeDef *hdma, HAL_DMA_CallbackIDTypeDef CallbackID,
                                           void (*pCallback)(DMA_HandleTypeDef *_hdma)) { return HAL_OK; }
HAL_StatusTypeDef HAL_DMA_UnRegisterCallback(DMA_HandleTypeDef *hdma, HAL_DMA_CallbackIDTypeDef CallbackID) { return HAL_OK; }
HAL_DMA_StateTypeDef HAL_DMA_GetState(DMA_HandleTypeDef *hdma) { return hdma->State; }
uint32_t HAL_DMA_GetError(DMA_HandleTypeDef *hdma) { return hdma->ErrorCode; }

/* --- ADC (simulation en phase 2) ------------------------------------------------------- */

HAL_StatusTypeDef HAL_ADC_Init(ADC_HandleTypeDef *hadc)
{
    if (hadc == NULL) {
        return HAL_ERROR;
    }
    PREBENCH_UNSIMULATED();
    if (hadc->State == HAL_ADC_STATE_RESET) {
        hadc->Lock = HAL_UNLOCKED;
        HAL_ADC_MspInit(hadc);
    }
    hadc->ErrorCode = HAL_ADC_ERROR_NONE;
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

HAL_StatusTypeDef HAL_ADC_Start(ADC_HandleTypeDef *hadc) { STUB_OK(); }
HAL_StatusTypeDef HAL_ADC_Stop(ADC_HandleTypeDef *hadc) { return HAL_OK; }
HAL_StatusTypeDef HAL_ADC_PollForConversion(ADC_HandleTypeDef *hadc, uint32_t Timeout) { STUB_OK(); }
HAL_StatusTypeDef HAL_ADC_PollForEvent(ADC_HandleTypeDef *hadc, uint32_t EventType, uint32_t Timeout)
{
    PREBENCH_UNSIMULATED();
    prebench_stub_wait(Timeout);
    return HAL_TIMEOUT;
}
HAL_StatusTypeDef HAL_ADC_Start_IT(ADC_HandleTypeDef *hadc) { STUB_OK(); }
HAL_StatusTypeDef HAL_ADC_Stop_IT(ADC_HandleTypeDef *hadc) { return HAL_OK; }
void HAL_ADC_IRQHandler(ADC_HandleTypeDef *hadc) {}
HAL_StatusTypeDef HAL_ADC_Start_DMA(ADC_HandleTypeDef *hadc, uint32_t *pData, uint32_t Length) { STUB_OK(); }
HAL_StatusTypeDef HAL_ADC_Stop_DMA(ADC_HandleTypeDef *hadc) { return HAL_OK; }
uint32_t HAL_ADC_GetValue(const ADC_HandleTypeDef *hadc)
{
    PREBENCH_UNSIMULATED();
    return 0U;
}
HAL_StatusTypeDef HAL_ADC_ConfigChannel(ADC_HandleTypeDef *hadc, const ADC_ChannelConfTypeDef *sConfig) { STUB_OK(); }
HAL_StatusTypeDef HAL_ADC_AnalogWDGConfig(ADC_HandleTypeDef *hadc, const ADC_AnalogWDGConfTypeDef *AnalogWDGConfig) { STUB_OK(); }
uint32_t HAL_ADC_GetState(const ADC_HandleTypeDef *hadc) { return hadc->State; }
uint32_t HAL_ADC_GetError(const ADC_HandleTypeDef *hadc) { return hadc->ErrorCode; }

/* --- I2C (simulation en phase 3) : bus sans composant ------------------------------------ */

HAL_StatusTypeDef HAL_I2C_Init(I2C_HandleTypeDef *hi2c)
{
    if (hi2c == NULL) {
        return HAL_ERROR;
    }
    PREBENCH_UNSIMULATED();
    if (hi2c->State == HAL_I2C_STATE_RESET) {
        hi2c->Lock = HAL_UNLOCKED;
        HAL_I2C_MspInit(hi2c);
    }
    hi2c->ErrorCode = HAL_I2C_ERROR_NONE;
    hi2c->State = HAL_I2C_STATE_READY;
    hi2c->Mode = HAL_I2C_MODE_NONE;
    return HAL_OK;
}

HAL_StatusTypeDef HAL_I2C_DeInit(I2C_HandleTypeDef *hi2c)
{
    if (hi2c == NULL) {
        return HAL_ERROR;
    }
    HAL_I2C_MspDeInit(hi2c);
    hi2c->State = HAL_I2C_STATE_RESET;
    return HAL_OK;
}

static HAL_StatusTypeDef i2c_nack(I2C_HandleTypeDef *hi2c, const char *name)
{
    vmcu_warn_unsimulated(name);
    hi2c->ErrorCode = HAL_I2C_ERROR_AF;
    return HAL_ERROR;
}

#define I2C_NACK() return i2c_nack(hi2c, __func__)

HAL_StatusTypeDef HAL_I2C_Master_Transmit(I2C_HandleTypeDef *hi2c, uint16_t DevAddress, uint8_t *pData, uint16_t Size, uint32_t Timeout) { I2C_NACK(); }
HAL_StatusTypeDef HAL_I2C_Master_Receive(I2C_HandleTypeDef *hi2c, uint16_t DevAddress, uint8_t *pData, uint16_t Size, uint32_t Timeout) { I2C_NACK(); }
HAL_StatusTypeDef HAL_I2C_Slave_Transmit(I2C_HandleTypeDef *hi2c, uint8_t *pData, uint16_t Size, uint32_t Timeout) { I2C_NACK(); }
HAL_StatusTypeDef HAL_I2C_Slave_Receive(I2C_HandleTypeDef *hi2c, uint8_t *pData, uint16_t Size, uint32_t Timeout) { I2C_NACK(); }
HAL_StatusTypeDef HAL_I2C_Mem_Write(I2C_HandleTypeDef *hi2c, uint16_t DevAddress, uint16_t MemAddress, uint16_t MemAddSize, uint8_t *pData, uint16_t Size, uint32_t Timeout) { I2C_NACK(); }
HAL_StatusTypeDef HAL_I2C_Mem_Read(I2C_HandleTypeDef *hi2c, uint16_t DevAddress, uint16_t MemAddress, uint16_t MemAddSize, uint8_t *pData, uint16_t Size, uint32_t Timeout) { I2C_NACK(); }
HAL_StatusTypeDef HAL_I2C_IsDeviceReady(I2C_HandleTypeDef *hi2c, uint16_t DevAddress, uint32_t Trials, uint32_t Timeout) { I2C_NACK(); }
HAL_StatusTypeDef HAL_I2C_Master_Transmit_IT(I2C_HandleTypeDef *hi2c, uint16_t DevAddress, uint8_t *pData, uint16_t Size) { I2C_NACK(); }
HAL_StatusTypeDef HAL_I2C_Master_Receive_IT(I2C_HandleTypeDef *hi2c, uint16_t DevAddress, uint8_t *pData, uint16_t Size) { I2C_NACK(); }
HAL_StatusTypeDef HAL_I2C_Slave_Transmit_IT(I2C_HandleTypeDef *hi2c, uint8_t *pData, uint16_t Size) { I2C_NACK(); }
HAL_StatusTypeDef HAL_I2C_Slave_Receive_IT(I2C_HandleTypeDef *hi2c, uint8_t *pData, uint16_t Size) { I2C_NACK(); }
HAL_StatusTypeDef HAL_I2C_Mem_Write_IT(I2C_HandleTypeDef *hi2c, uint16_t DevAddress, uint16_t MemAddress, uint16_t MemAddSize, uint8_t *pData, uint16_t Size) { I2C_NACK(); }
HAL_StatusTypeDef HAL_I2C_Mem_Read_IT(I2C_HandleTypeDef *hi2c, uint16_t DevAddress, uint16_t MemAddress, uint16_t MemAddSize, uint8_t *pData, uint16_t Size) { I2C_NACK(); }
HAL_StatusTypeDef HAL_I2C_Master_Transmit_DMA(I2C_HandleTypeDef *hi2c, uint16_t DevAddress, uint8_t *pData, uint16_t Size) { I2C_NACK(); }
HAL_StatusTypeDef HAL_I2C_Master_Receive_DMA(I2C_HandleTypeDef *hi2c, uint16_t DevAddress, uint8_t *pData, uint16_t Size) { I2C_NACK(); }
HAL_StatusTypeDef HAL_I2C_Slave_Transmit_DMA(I2C_HandleTypeDef *hi2c, uint8_t *pData, uint16_t Size) { I2C_NACK(); }
HAL_StatusTypeDef HAL_I2C_Slave_Receive_DMA(I2C_HandleTypeDef *hi2c, uint8_t *pData, uint16_t Size) { I2C_NACK(); }
HAL_StatusTypeDef HAL_I2C_Mem_Write_DMA(I2C_HandleTypeDef *hi2c, uint16_t DevAddress, uint16_t MemAddress, uint16_t MemAddSize, uint8_t *pData, uint16_t Size) { I2C_NACK(); }
HAL_StatusTypeDef HAL_I2C_Mem_Read_DMA(I2C_HandleTypeDef *hi2c, uint16_t DevAddress, uint16_t MemAddress, uint16_t MemAddSize, uint8_t *pData, uint16_t Size) { I2C_NACK(); }
HAL_StatusTypeDef HAL_I2C_EnableListen_IT(I2C_HandleTypeDef *hi2c) { STUB_OK(); }
HAL_StatusTypeDef HAL_I2C_DisableListen_IT(I2C_HandleTypeDef *hi2c) { return HAL_OK; }
HAL_StatusTypeDef HAL_I2C_Master_Abort_IT(I2C_HandleTypeDef *hi2c, uint16_t DevAddress) { return HAL_OK; }
void HAL_I2C_EV_IRQHandler(I2C_HandleTypeDef *hi2c) {}
void HAL_I2C_ER_IRQHandler(I2C_HandleTypeDef *hi2c) {}
HAL_I2C_StateTypeDef HAL_I2C_GetState(const I2C_HandleTypeDef *hi2c) { return hi2c->State; }
HAL_I2C_ModeTypeDef HAL_I2C_GetMode(const I2C_HandleTypeDef *hi2c) { return hi2c->Mode; }
uint32_t HAL_I2C_GetError(const I2C_HandleTypeDef *hi2c) { return hi2c->ErrorCode; }
HAL_StatusTypeDef HAL_I2CEx_ConfigAnalogFilter(I2C_HandleTypeDef *hi2c, uint32_t AnalogFilter) { return HAL_OK; }
HAL_StatusTypeDef HAL_I2CEx_ConfigDigitalFilter(I2C_HandleTypeDef *hi2c, uint32_t DigitalFilter) { return HAL_OK; }

/* --- SPI (simulation en phase 3) : MISO sans esclave = 0xFF ------------------------------- */

HAL_StatusTypeDef HAL_SPI_Init(SPI_HandleTypeDef *hspi)
{
    if (hspi == NULL) {
        return HAL_ERROR;
    }
    PREBENCH_UNSIMULATED();
    if (hspi->State == HAL_SPI_STATE_RESET) {
        hspi->Lock = HAL_UNLOCKED;
        HAL_SPI_MspInit(hspi);
    }
    hspi->ErrorCode = HAL_SPI_ERROR_NONE;
    hspi->State = HAL_SPI_STATE_READY;
    return HAL_OK;
}

HAL_StatusTypeDef HAL_SPI_DeInit(SPI_HandleTypeDef *hspi)
{
    if (hspi == NULL) {
        return HAL_ERROR;
    }
    HAL_SPI_MspDeInit(hspi);
    hspi->State = HAL_SPI_STATE_RESET;
    return HAL_OK;
}

static void spi_fill_rx(const SPI_HandleTypeDef *hspi, uint8_t *rx, uint16_t size)
{
    if (rx != NULL) {
        size_t bytes = (size_t)size * ((hspi->Init.DataSize == SPI_DATASIZE_16BIT) ? 2U : 1U);
        memset(rx, 0xFF, bytes);
    }
}

HAL_StatusTypeDef HAL_SPI_Transmit(SPI_HandleTypeDef *hspi, const uint8_t *pData, uint16_t Size, uint32_t Timeout) { STUB_OK(); }
HAL_StatusTypeDef HAL_SPI_Receive(SPI_HandleTypeDef *hspi, uint8_t *pData, uint16_t Size, uint32_t Timeout)
{
    PREBENCH_UNSIMULATED();
    spi_fill_rx(hspi, pData, Size);
    return HAL_OK;
}
HAL_StatusTypeDef HAL_SPI_TransmitReceive(SPI_HandleTypeDef *hspi, const uint8_t *pTxData, uint8_t *pRxData, uint16_t Size, uint32_t Timeout)
{
    PREBENCH_UNSIMULATED();
    spi_fill_rx(hspi, pRxData, Size);
    return HAL_OK;
}
HAL_StatusTypeDef HAL_SPI_Transmit_IT(SPI_HandleTypeDef *hspi, const uint8_t *pData, uint16_t Size) { STUB_OK(); }
HAL_StatusTypeDef HAL_SPI_Receive_IT(SPI_HandleTypeDef *hspi, uint8_t *pData, uint16_t Size) { STUB_OK(); }
HAL_StatusTypeDef HAL_SPI_TransmitReceive_IT(SPI_HandleTypeDef *hspi, const uint8_t *pTxData, uint8_t *pRxData, uint16_t Size) { STUB_OK(); }
HAL_StatusTypeDef HAL_SPI_Transmit_DMA(SPI_HandleTypeDef *hspi, const uint8_t *pData, uint16_t Size) { STUB_OK(); }
HAL_StatusTypeDef HAL_SPI_Receive_DMA(SPI_HandleTypeDef *hspi, uint8_t *pData, uint16_t Size) { STUB_OK(); }
HAL_StatusTypeDef HAL_SPI_TransmitReceive_DMA(SPI_HandleTypeDef *hspi, const uint8_t *pTxData, uint8_t *pRxData, uint16_t Size) { STUB_OK(); }
HAL_StatusTypeDef HAL_SPI_DMAPause(SPI_HandleTypeDef *hspi) { return HAL_OK; }
HAL_StatusTypeDef HAL_SPI_DMAResume(SPI_HandleTypeDef *hspi) { return HAL_OK; }
HAL_StatusTypeDef HAL_SPI_DMAStop(SPI_HandleTypeDef *hspi) { return HAL_OK; }
HAL_StatusTypeDef HAL_SPI_Abort(SPI_HandleTypeDef *hspi) { return HAL_OK; }
HAL_StatusTypeDef HAL_SPI_Abort_IT(SPI_HandleTypeDef *hspi) { return HAL_OK; }
void HAL_SPI_IRQHandler(SPI_HandleTypeDef *hspi) {}
HAL_SPI_StateTypeDef HAL_SPI_GetState(const SPI_HandleTypeDef *hspi) { return hspi->State; }
uint32_t HAL_SPI_GetError(const SPI_HandleTypeDef *hspi) { return hspi->ErrorCode; }
