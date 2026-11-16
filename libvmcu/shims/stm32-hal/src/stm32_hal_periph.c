/*
 * libvmcu/shims/stm32-hal/src/stm32_hal_periph.c — Instances des périphériques et
 * modules DMA et SPI pas encore simulés (règle 16.2.10). UART, TIM (PWM), ADC et
 * I2C sont simulés dans stm32_hal_uart.c, stm32_hal_tim.c, stm32_hal_adc.c et
 * stm32_hal_i2c.c.
 *
 * Comportement commun :
 * - *_Init : appelle le MspInit du module si le handle est neuf (comme le HAL de
 *   ST : CubeMX y configure les broches), passe à l'état READY, renvoie HAL_OK ;
 * - émissions : acceptées sans effet ;
 * - réceptions bloquantes : attente bornée à 10 ms puis HAL_TIMEOUT ;
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
