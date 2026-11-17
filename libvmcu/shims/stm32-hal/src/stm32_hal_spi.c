/*
 * libvmcu/shims/stm32-hal/src/stm32_hal_spi.c — SPI maître du HAL STM32 sur
 * libvmcu (vmcu_spi_*) ; phase 3, étape 6.
 *
 * Bus : numéro du périphérique (SPI1 -> 1), dérivé de hspi->Instance.
 * CS : piloté par le firmware en GPIO (NSS logiciel, cas de CubeMX) ; le cœur
 * retrouve le composant sélectionné d'après l'état des nets (VMCU_PIN_NONE).
 * - Transmit : écriture seule, sans attente (un message par appel, en mémoire
 *   partagée au-delà de 4096 octets) ; les écritures de broches qui suivent
 *   (CS relâché) partent après elle, dans l'ordre.
 * - Receive, TransmitReceive : transaction synchrone, le firmware attend la
 *   réponse du modèle. Receive émet 0xFF.
 * - Variantes _IT et _DMA : transaction immédiate, puis callbacks de fin
 *   (_DMA : demi-transfert puis fin, comme le HAL) depuis le fil IRQ
 *   (vmcu_irq_post), comme l'I2C. Un callback peut relancer un transfert.
 * - Trames de 16 bits : chaque mot part poids fort d'abord (FirstBit MSB) ou
 *   poids faible d'abord (LSB) ; Size compte des mots.
 * Limites : pas de mode esclave, ni de NSS matériel, ni de CRC, ni de mode TI ;
 * ces cas avertissent « non simulé ».
 */
#include "stm32_hal_internal.h"

#include <stddef.h>
#include <stdlib.h>
#include <string.h>

#if defined(__GNUC__) || defined(__clang__)
#pragma GCC diagnostic ignored "-Wunused-parameter"
#endif

#define SPI_COUNT 5

/** Numéro du bus, ou -1 si le handle ne désigne pas un SPI connu. */
static int spi_bus(const SPI_HandleTypeDef *hspi)
{
    if (hspi == NULL || hspi->Instance == NULL) {
        return -1;
    }
    ptrdiff_t i = hspi->Instance - prebench_spi;
    return (i > 0 && i < SPI_COUNT) ? (int)i : -1;
}

static bool wide(const SPI_HandleTypeDef *hspi)
{
    return hspi->Init.DataSize == SPI_DATASIZE_16BIT;
}

static bool lsb_first(const SPI_HandleTypeDef *hspi)
{
    return hspi->Init.FirstBit == SPI_FIRSTBIT_LSB;
}

/** Fréquence de l'horloge SCK : PCLK du bus (APB2 pour SPI1 et SPI4) / prédiviseur. */
static uint32_t sck_hz(const SPI_HandleTypeDef *hspi, int bus)
{
    uint32_t pclk = (bus == 1 || bus == 4) ? HAL_RCC_GetPCLK2Freq() : HAL_RCC_GetPCLK1Freq();
    uint32_t shift = ((hspi->Init.BaudRatePrescaler >> 3) & 0x7u) + 1u; /* /2 à /256 */
    return pclk >> shift;
}

HAL_StatusTypeDef HAL_SPI_Init(SPI_HandleTypeDef *hspi)
{
    int bus = spi_bus(hspi);
    if (bus < 0) {
        return HAL_ERROR;
    }
    if (hspi->State == HAL_SPI_STATE_RESET) {
        hspi->Lock = HAL_UNLOCKED;
        HAL_SPI_MspInit(hspi);
    }
    if (hspi->Init.Mode != SPI_MODE_MASTER) {
        vmcu_warn_unsimulated("HAL_SPI_Init (mode esclave)");
    }
    if (hspi->Init.NSS == SPI_NSS_HARD_OUTPUT) {
        vmcu_warn_unsimulated("HAL_SPI_Init (NSS matériel : piloter CS en GPIO)");
    }
    if (hspi->Init.CRCCalculation == SPI_CRCCALCULATION_ENABLE) {
        vmcu_warn_unsimulated("HAL_SPI_Init (CRC)");
    }
    if (hspi->Init.TIMode == SPI_TIMODE_ENABLE) {
        vmcu_warn_unsimulated("HAL_SPI_Init (mode TI)");
    }
    vmcu_spi_mode_t mode = (vmcu_spi_mode_t)(((hspi->Init.CLKPolarity == SPI_POLARITY_HIGH) ? 2 : 0)
                                             + ((hspi->Init.CLKPhase == SPI_PHASE_2EDGE) ? 1 : 0));
    (void)vmcu_spi_config((uint8_t)bus, mode, sck_hz(hspi, bus), lsb_first(hspi));
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

/* --- Transaction ------------------------------------------------------------------------ */

static HAL_StatusTypeDef to_hal(SPI_HandleTypeDef *hspi, vmcu_status_t st)
{
    switch (st) {
    case VMCU_OK:
        hspi->ErrorCode = HAL_SPI_ERROR_NONE;
        return HAL_OK;
    case VMCU_ERR_TIMEOUT:
        hspi->ErrorCode = HAL_SPI_ERROR_FLAG;
        return HAL_TIMEOUT;
    default:
        hspi->ErrorCode = HAL_SPI_ERROR_FLAG;
        return HAL_ERROR;
    }
}

/** Mots de 16 bits <-> octets dans l'ordre du fil. */
static void words_to_wire(const uint8_t *mem, uint8_t *wire, size_t words, bool lsb)
{
    for (size_t i = 0; i < words; ++i) {
        uint16_t v;
        memcpy(&v, mem + 2 * i, sizeof v);
        wire[2 * i] = (uint8_t)(lsb ? v : v >> 8);
        wire[2 * i + 1] = (uint8_t)(lsb ? v >> 8 : v);
    }
}

static void wire_to_words(const uint8_t *wire, uint8_t *mem, size_t words, bool lsb)
{
    for (size_t i = 0; i < words; ++i) {
        uint16_t v = lsb ? (uint16_t)(wire[2 * i] | (wire[2 * i + 1] << 8))
                         : (uint16_t)((wire[2 * i] << 8) | wire[2 * i + 1]);
        memcpy(mem + 2 * i, &v, sizeof v);
    }
}

/**
 * Une transaction de `size` trames (octets ou mots). tx NULL : 0xFF émis ;
 * rx NULL : écriture seule, sans attente.
 */
static vmcu_status_t xfer(SPI_HandleTypeDef *hspi, const uint8_t *tx, uint8_t *rx, uint16_t size)
{
    int bus = spi_bus(hspi);
    if (bus < 0 || hspi->Init.Mode != SPI_MODE_MASTER) {
        return VMCU_ERR_UNSUPPORTED;
    }
    if (!wide(hspi)) {
        return vmcu_spi_xfer((uint8_t)bus, VMCU_PIN_NONE, VMCU_PIN_NONE, tx, rx, size, 0);
    }
    /* 16 bits : tampons de conversion, bornés par Size (65 535 mots). */
    size_t bytes = (size_t)size * 2u;
    uint8_t *wire_tx = tx != NULL ? malloc(bytes) : NULL;
    uint8_t *wire_rx = rx != NULL ? malloc(bytes) : NULL;
    vmcu_status_t st = VMCU_ERR_NO_MEMORY;
    if ((tx == NULL || wire_tx != NULL) && (rx == NULL || wire_rx != NULL)) {
        if (tx != NULL) {
            words_to_wire(tx, wire_tx, size, lsb_first(hspi));
        }
        st = vmcu_spi_xfer((uint8_t)bus, VMCU_PIN_NONE, VMCU_PIN_NONE, wire_tx, wire_rx, bytes, 0);
        if (st == VMCU_OK && rx != NULL) {
            wire_to_words(wire_rx, rx, size, lsb_first(hspi));
        }
    }
    free(wire_tx);
    free(wire_rx);
    return st;
}

static void warn_unsupported(const SPI_HandleTypeDef *hspi)
{
    if (hspi->Init.Mode != SPI_MODE_MASTER) {
        vmcu_warn_unsimulated("HAL_SPI (mode esclave)");
    }
}

/* --- Transferts bloquants --------------------------------------------------------------- */

static HAL_StatusTypeDef blocking(SPI_HandleTypeDef *hspi, const uint8_t *tx, uint8_t *rx,
                                  uint16_t size)
{
    if (spi_bus(hspi) < 0 || size == 0 || (tx == NULL && rx == NULL)) {
        return HAL_ERROR;
    }
    if (hspi->State != HAL_SPI_STATE_READY) {
        return HAL_BUSY;
    }
    hspi->State = rx == NULL ? HAL_SPI_STATE_BUSY_TX
                  : tx == NULL ? HAL_SPI_STATE_BUSY_RX : HAL_SPI_STATE_BUSY_TX_RX;
    vmcu_status_t st = xfer(hspi, tx, rx, size);
    hspi->State = HAL_SPI_STATE_READY;
    if (st == VMCU_ERR_UNSUPPORTED) {
        warn_unsupported(hspi);
    }
    return to_hal(hspi, st);
}

HAL_StatusTypeDef HAL_SPI_Transmit(SPI_HandleTypeDef *hspi, const uint8_t *pData, uint16_t Size, uint32_t Timeout)
{
    return blocking(hspi, pData, NULL, Size);
}

HAL_StatusTypeDef HAL_SPI_Receive(SPI_HandleTypeDef *hspi, uint8_t *pData, uint16_t Size, uint32_t Timeout)
{
    return blocking(hspi, NULL, pData, Size);
}

HAL_StatusTypeDef HAL_SPI_TransmitReceive(SPI_HandleTypeDef *hspi, const uint8_t *pTxData, uint8_t *pRxData, uint16_t Size, uint32_t Timeout)
{
    if (pTxData == NULL || pRxData == NULL) {
        return HAL_ERROR;
    }
    return blocking(hspi, pTxData, pRxData, Size);
}

/* --- Variantes _IT et _DMA : transaction immédiate, callbacks depuis le fil IRQ -------- */

/* Fil IRQ : fin d'un transfert. State dit quel callback appeler ; sans
 * routine d'interruption (TxISR NULL), c'est un transfert DMA : le
 * demi-transfert est annoncé d'abord. */
static void xfer_done(void *arg)
{
    SPI_HandleTypeDef *hspi = (SPI_HandleTypeDef *)arg;
    HAL_SPI_StateTypeDef state = hspi->State;
    bool dma = hspi->TxISR == NULL;
    hspi->TxXferCount = 0;
    hspi->RxXferCount = 0;
    hspi->State = HAL_SPI_STATE_READY;
    if (hspi->ErrorCode != HAL_SPI_ERROR_NONE) {
        HAL_SPI_ErrorCallback(hspi);
        return;
    }
    switch (state) {
    case HAL_SPI_STATE_BUSY_TX:
        if (dma) {
            HAL_SPI_TxHalfCpltCallback(hspi);
        }
        HAL_SPI_TxCpltCallback(hspi);
        break;
    case HAL_SPI_STATE_BUSY_RX:
        if (dma) {
            HAL_SPI_RxHalfCpltCallback(hspi);
        }
        HAL_SPI_RxCpltCallback(hspi);
        break;
    default:
        if (dma) {
            HAL_SPI_TxRxHalfCpltCallback(hspi);
        }
        HAL_SPI_TxRxCpltCallback(hspi);
        break;
    }
}

static void it_marker(SPI_HandleTypeDef *hspi)
{
    (void)hspi; /* marqueur : transfert par interruption */
}

static HAL_StatusTypeDef async(SPI_HandleTypeDef *hspi, const uint8_t *tx, uint8_t *rx,
                               uint16_t size, bool dma)
{
    if (spi_bus(hspi) < 0 || size == 0 || (tx == NULL && rx == NULL)) {
        return HAL_ERROR;
    }
    if (hspi->State != HAL_SPI_STATE_READY) {
        return HAL_BUSY;
    }
    hspi->State = rx == NULL ? HAL_SPI_STATE_BUSY_TX
                  : tx == NULL ? HAL_SPI_STATE_BUSY_RX : HAL_SPI_STATE_BUSY_TX_RX;
    hspi->pTxBuffPtr = tx;
    hspi->TxXferSize = tx != NULL ? size : 0;
    hspi->TxXferCount = hspi->TxXferSize;
    hspi->pRxBuffPtr = rx;
    hspi->RxXferSize = rx != NULL ? size : 0;
    hspi->RxXferCount = hspi->RxXferSize;
    /* _IT : routines d'interruption renseignées ; _DMA : aucune (callbacks de demi-transfert). */
    hspi->TxISR = dma ? NULL : it_marker;
    hspi->RxISR = dma ? NULL : it_marker;
    vmcu_status_t st = xfer(hspi, tx, rx, size);
    if (st == VMCU_ERR_UNSUPPORTED) {
        warn_unsupported(hspi);
        hspi->State = HAL_SPI_STATE_READY;
        (void)to_hal(hspi, st);
        return HAL_ERROR;
    }
    (void)to_hal(hspi, st); /* délai ou erreur : remonté par HAL_SPI_ErrorCallback */
    if (vmcu_irq_post(xfer_done, hspi) != VMCU_OK) {
        hspi->State = HAL_SPI_STATE_READY;
        return HAL_BUSY;
    }
    return HAL_OK;
}

HAL_StatusTypeDef HAL_SPI_Transmit_IT(SPI_HandleTypeDef *hspi, const uint8_t *pData, uint16_t Size)
{
    return async(hspi, pData, NULL, Size, false);
}

HAL_StatusTypeDef HAL_SPI_Receive_IT(SPI_HandleTypeDef *hspi, uint8_t *pData, uint16_t Size)
{
    return async(hspi, NULL, pData, Size, false);
}

HAL_StatusTypeDef HAL_SPI_TransmitReceive_IT(SPI_HandleTypeDef *hspi, const uint8_t *pTxData, uint8_t *pRxData, uint16_t Size)
{
    if (pTxData == NULL || pRxData == NULL) {
        return HAL_ERROR;
    }
    return async(hspi, pTxData, pRxData, Size, false);
}

HAL_StatusTypeDef HAL_SPI_Transmit_DMA(SPI_HandleTypeDef *hspi, const uint8_t *pData, uint16_t Size)
{
    return async(hspi, pData, NULL, Size, true);
}

HAL_StatusTypeDef HAL_SPI_Receive_DMA(SPI_HandleTypeDef *hspi, uint8_t *pData, uint16_t Size)
{
    return async(hspi, NULL, pData, Size, true);
}

HAL_StatusTypeDef HAL_SPI_TransmitReceive_DMA(SPI_HandleTypeDef *hspi, const uint8_t *pTxData, uint8_t *pRxData, uint16_t Size)
{
    if (pTxData == NULL || pRxData == NULL) {
        return HAL_ERROR;
    }
    return async(hspi, pTxData, pRxData, Size, true);
}

/* --- Divers ------------------------------------------------------------------------------ */

/* Les transferts sont déjà terminés quand l'appel rend la main : rien à suspendre ni à annuler. */
HAL_StatusTypeDef HAL_SPI_DMAPause(SPI_HandleTypeDef *hspi) { return HAL_OK; }
HAL_StatusTypeDef HAL_SPI_DMAResume(SPI_HandleTypeDef *hspi) { return HAL_OK; }
HAL_StatusTypeDef HAL_SPI_DMAStop(SPI_HandleTypeDef *hspi) { return HAL_OK; }
HAL_StatusTypeDef HAL_SPI_Abort(SPI_HandleTypeDef *hspi) { return HAL_OK; }

HAL_StatusTypeDef HAL_SPI_Abort_IT(SPI_HandleTypeDef *hspi)
{
    if (hspi == NULL) {
        return HAL_ERROR;
    }
    HAL_SPI_AbortCpltCallback(hspi);
    return HAL_OK;
}

void HAL_SPI_IRQHandler(SPI_HandleTypeDef *hspi) {}
HAL_SPI_StateTypeDef HAL_SPI_GetState(const SPI_HandleTypeDef *hspi) { return hspi->State; }
uint32_t HAL_SPI_GetError(const SPI_HandleTypeDef *hspi) { return hspi->ErrorCode; }
