/*
 * libvmcu/shims/stm32-hal/src/stm32_hal_uart.c — UART du HAL STM32 sur libvmcu (vmcu_uart_*).
 *
 * Identifiant : numéro du périphérique (USART2 -> 2), dérivé de huart->Instance.
 * - HAL_UART_Init : débit annoncé à l'app (UART_CONFIG ; alerte de débit du moniteur).
 * - Émission bloquante, _IT et _DMA : octets envoyés aussitôt ; pour _IT et _DMA,
 *   HAL_UART_TxCpltCallback est appelé ensuite depuis le fil IRQ (vmcu_irq_post).
 * - Réception bloquante : attend les octets jusqu'au délai (HAL_TIMEOUT sinon).
 * - Réception _IT et _DMA : HAL_UART_RxCpltCallback quand Size octets sont reçus,
 *   depuis le fil IRQ ; une réception relancée dans le callback reçoit la suite.
 * - ReceiveToIdle (_IT, _DMA) : HAL_UARTEx_RxEventCallback à chaque paquet reçu
 *   de l'app (le silence de ligne d'une vraie liaison), ou quand Size est atteint.
 * Limites : pas de DMA circulaire (reçu comme une réception simple), pas de
 * format 9 bits ni de parité, pas d'accès aux registres.
 */
#include "stm32_hal_internal.h"

#include <stddef.h>

#if defined(__GNUC__) || defined(__clang__)
#pragma GCC diagnostic ignored "-Wunused-parameter"
#endif

#define UART_COUNT 7

/** Numéro du périphérique, ou -1 si le handle ne désigne pas un USART connu. */
static int uart_id(const UART_HandleTypeDef *huart)
{
    if (huart == NULL || huart->Instance == NULL) {
        return -1;
    }
    ptrdiff_t i = huart->Instance - prebench_usart;
    return (i > 0 && i < UART_COUNT) ? (int)i : -1;
}

static HAL_StatusTypeDef uart_init(UART_HandleTypeDef *huart, const char *partial)
{
    int id = uart_id(huart);
    if (id < 0) {
        return HAL_ERROR;
    }
    if (partial != NULL) {
        vmcu_warn_unsimulated(partial); /* mode particulier : transmis comme une UART simple */
    }
    if (huart->gState == HAL_UART_STATE_RESET) {
        huart->Lock = HAL_UNLOCKED;
        HAL_UART_MspInit(huart);
    }
    (void)vmcu_uart_config((uint8_t)id, huart->Init.BaudRate);
    huart->ErrorCode = HAL_UART_ERROR_NONE;
    huart->gState = HAL_UART_STATE_READY;
    huart->RxState = HAL_UART_STATE_READY;
    huart->ReceptionType = HAL_UART_RECEPTION_STANDARD;
    return HAL_OK;
}

HAL_StatusTypeDef HAL_UART_Init(UART_HandleTypeDef *huart) { return uart_init(huart, NULL); }
HAL_StatusTypeDef HAL_HalfDuplex_Init(UART_HandleTypeDef *huart) { return uart_init(huart, "HAL_HalfDuplex_Init"); }
HAL_StatusTypeDef HAL_LIN_Init(UART_HandleTypeDef *huart, uint32_t BreakDetectLength) { return uart_init(huart, "HAL_LIN_Init"); }
HAL_StatusTypeDef HAL_MultiProcessor_Init(UART_HandleTypeDef *huart, uint8_t Address, uint32_t WakeUpMethod)
{
    return uart_init(huart, "HAL_MultiProcessor_Init");
}

static void stop_rx(UART_HandleTypeDef *huart)
{
    int id = uart_id(huart);
    if (id >= 0) {
        (void)vmcu_uart_on_rx((uint8_t)id, NULL, NULL);
    }
    huart->RxState = HAL_UART_STATE_READY;
    huart->ReceptionType = HAL_UART_RECEPTION_STANDARD;
}

HAL_StatusTypeDef HAL_UART_DeInit(UART_HandleTypeDef *huart)
{
    if (uart_id(huart) < 0) {
        return HAL_ERROR;
    }
    stop_rx(huart);
    HAL_UART_MspDeInit(huart);
    huart->gState = HAL_UART_STATE_RESET;
    huart->RxState = HAL_UART_STATE_RESET;
    return HAL_OK;
}

FlagStatus prebench_uart_get_flag(const UART_HandleTypeDef *huart, uint32_t flag)
{
    if ((flag & (UART_FLAG_TXE | UART_FLAG_TC)) != 0U) {
        return SET; /* émission immédiate */
    }
    int id = uart_id(huart);
    if ((flag & UART_FLAG_RXNE) != 0U && id >= 0) {
        return vmcu_uart_available((uint8_t)id) > 0 ? SET : RESET;
    }
    return RESET;
}

/* --- Émission ------------------------------------------------------------------------- */

HAL_StatusTypeDef HAL_UART_Transmit(UART_HandleTypeDef *huart, const uint8_t *pData, uint16_t Size, uint32_t Timeout)
{
    int id = uart_id(huart);
    if (id < 0 || (pData == NULL && Size > 0)) {
        return HAL_ERROR;
    }
    return vmcu_uart_write((uint8_t)id, pData, Size) == VMCU_OK ? HAL_OK : HAL_ERROR;
}

static void tx_done(void *arg)
{
    UART_HandleTypeDef *huart = (UART_HandleTypeDef *)arg;
    huart->TxXferCount = 0;
    huart->gState = HAL_UART_STATE_READY;
    HAL_UART_TxCpltCallback(huart);
}

static HAL_StatusTypeDef transmit_async(UART_HandleTypeDef *huart, const uint8_t *pData, uint16_t Size)
{
    int id = uart_id(huart);
    if (id < 0 || pData == NULL || Size == 0) {
        return HAL_ERROR;
    }
    if (huart->gState != HAL_UART_STATE_READY) {
        return HAL_BUSY;
    }
    huart->pTxBuffPtr = pData;
    huart->TxXferSize = Size;
    huart->TxXferCount = Size;
    huart->gState = HAL_UART_STATE_BUSY_TX;
    if (vmcu_uart_write((uint8_t)id, pData, Size) != VMCU_OK) {
        huart->gState = HAL_UART_STATE_READY;
        return HAL_ERROR;
    }
    if (vmcu_irq_post(tx_done, huart) != VMCU_OK) {
        huart->gState = HAL_UART_STATE_READY;
        return HAL_BUSY;
    }
    return HAL_OK;
}

HAL_StatusTypeDef HAL_UART_Transmit_IT(UART_HandleTypeDef *huart, const uint8_t *pData, uint16_t Size)
{
    return transmit_async(huart, pData, Size);
}

HAL_StatusTypeDef HAL_UART_Transmit_DMA(UART_HandleTypeDef *huart, const uint8_t *pData, uint16_t Size)
{
    return transmit_async(huart, pData, Size);
}

/* --- Réception ------------------------------------------------------------------------ */

/** Délai restant d'une attente HAL (HAL_MAX_DELAY : sans limite). */
static uint32_t remaining(uint32_t start, uint32_t timeout)
{
    if (timeout == HAL_MAX_DELAY) {
        return 1000u; /* par tranches, sans fin */
    }
    uint32_t elapsed = HAL_GetTick() - start;
    return (elapsed >= timeout) ? 0u : timeout - elapsed;
}

HAL_StatusTypeDef HAL_UART_Receive(UART_HandleTypeDef *huart, uint8_t *pData, uint16_t Size, uint32_t Timeout)
{
    int id = uart_id(huart);
    if (id < 0 || pData == NULL || Size == 0) {
        return HAL_ERROR;
    }
    if (huart->RxState != HAL_UART_STATE_READY) {
        return HAL_BUSY;
    }
    huart->RxState = HAL_UART_STATE_BUSY_RX;
    uint32_t start = HAL_GetTick();
    uint16_t got = 0;
    for (;;) {
        got += (uint16_t)vmcu_uart_read((uint8_t)id, pData + got, (size_t)(Size - got), remaining(start, Timeout));
        if (got == Size) {
            break;
        }
        if (remaining(start, Timeout) == 0u) {
            huart->RxState = HAL_UART_STATE_READY;
            return HAL_TIMEOUT;
        }
    }
    huart->RxState = HAL_UART_STATE_READY;
    return HAL_OK;
}

HAL_StatusTypeDef HAL_UARTEx_ReceiveToIdle(UART_HandleTypeDef *huart, uint8_t *pData, uint16_t Size, uint16_t *RxLen,
                                           uint32_t Timeout)
{
    int id = uart_id(huart);
    if (RxLen != NULL) {
        *RxLen = 0;
    }
    if (id < 0 || pData == NULL || Size == 0) {
        return HAL_ERROR;
    }
    uint32_t start = HAL_GetTick();
    size_t got = 0;
    while (got == 0) {
        got = vmcu_uart_read((uint8_t)id, pData, Size, remaining(start, Timeout));
        if (got == 0 && remaining(start, Timeout) == 0u) {
            return HAL_TIMEOUT;
        }
    }
    /* Ligne silencieuse : plus rien pendant 2 ms. */
    while (got < Size) {
        size_t more = vmcu_uart_read((uint8_t)id, pData + got, Size - got, 2u);
        if (more == 0) {
            break;
        }
        got += more;
    }
    huart->RxEventType = (got == Size) ? HAL_UART_RXEVENT_TC : HAL_UART_RXEVENT_IDLE;
    if (RxLen != NULL) {
        *RxLen = (uint16_t)got;
    }
    return HAL_OK;
}

/* Fil IRQ : octets reçus pendant une réception _IT ou _DMA. */
static void rx_bytes(uint8_t id, const uint8_t *data, size_t len, void *user)
{
    UART_HandleTypeDef *huart = (UART_HandleTypeDef *)user;
    size_t i = 0;
    while (i < len && huart->RxState == HAL_UART_STATE_BUSY_RX) {
        while (i < len && huart->RxXferCount > 0) {
            huart->pRxBuffPtr[huart->RxXferSize - huart->RxXferCount] = data[i++];
            huart->RxXferCount--;
        }
        uint16_t received = (uint16_t)(huart->RxXferSize - huart->RxXferCount);
        if (huart->ReceptionType == HAL_UART_RECEPTION_TOIDLE) {
            /* Fin du paquet (silence de ligne) ou tampon plein. */
            if (i == len || huart->RxXferCount == 0) {
                huart->RxEventType = (huart->RxXferCount == 0) ? HAL_UART_RXEVENT_TC : HAL_UART_RXEVENT_IDLE;
                stop_rx(huart);
                HAL_UARTEx_RxEventCallback(huart, received);
            }
        } else if (huart->RxXferCount == 0) {
            stop_rx(huart);
            HAL_UART_RxCpltCallback(huart); /* peut relancer une réception : la suite y va */
        }
    }
    if (i < len) {
        huart->ErrorCode |= HAL_UART_ERROR_ORE; /* aucune réception en cours : octets perdus */
    }
    (void)id;
}

static HAL_StatusTypeDef receive_async(UART_HandleTypeDef *huart, uint8_t *pData, uint16_t Size, uint32_t type)
{
    int id = uart_id(huart);
    if (id < 0 || pData == NULL || Size == 0) {
        return HAL_ERROR;
    }
    if (huart->RxState != HAL_UART_STATE_READY) {
        return HAL_BUSY;
    }
    huart->pRxBuffPtr = pData;
    huart->RxXferSize = Size;
    huart->RxXferCount = Size;
    huart->ReceptionType = type;
    huart->RxState = HAL_UART_STATE_BUSY_RX;
    return vmcu_uart_on_rx((uint8_t)id, rx_bytes, huart) == VMCU_OK ? HAL_OK : HAL_ERROR;
}

HAL_StatusTypeDef HAL_UART_Receive_IT(UART_HandleTypeDef *huart, uint8_t *pData, uint16_t Size)
{
    return receive_async(huart, pData, Size, HAL_UART_RECEPTION_STANDARD);
}

HAL_StatusTypeDef HAL_UART_Receive_DMA(UART_HandleTypeDef *huart, uint8_t *pData, uint16_t Size)
{
    return receive_async(huart, pData, Size, HAL_UART_RECEPTION_STANDARD);
}

HAL_StatusTypeDef HAL_UARTEx_ReceiveToIdle_IT(UART_HandleTypeDef *huart, uint8_t *pData, uint16_t Size)
{
    return receive_async(huart, pData, Size, HAL_UART_RECEPTION_TOIDLE);
}

HAL_StatusTypeDef HAL_UARTEx_ReceiveToIdle_DMA(UART_HandleTypeDef *huart, uint8_t *pData, uint16_t Size)
{
    return receive_async(huart, pData, Size, HAL_UART_RECEPTION_TOIDLE);
}

HAL_UART_RxEventTypeTypeDef HAL_UARTEx_GetRxEventType(UART_HandleTypeDef *huart) { return huart->RxEventType; }

/* --- Arrêts, état ----------------------------------------------------------------------- */

HAL_StatusTypeDef HAL_UART_DMAPause(UART_HandleTypeDef *huart) { return HAL_OK; }
HAL_StatusTypeDef HAL_UART_DMAResume(UART_HandleTypeDef *huart) { return HAL_OK; }

HAL_StatusTypeDef HAL_UART_DMAStop(UART_HandleTypeDef *huart)
{
    stop_rx(huart);
    huart->gState = HAL_UART_STATE_READY;
    return HAL_OK;
}

HAL_StatusTypeDef HAL_UART_Abort(UART_HandleTypeDef *huart) { return HAL_UART_DMAStop(huart); }

HAL_StatusTypeDef HAL_UART_AbortTransmit(UART_HandleTypeDef *huart)
{
    huart->gState = HAL_UART_STATE_READY;
    return HAL_OK;
}

HAL_StatusTypeDef HAL_UART_AbortReceive(UART_HandleTypeDef *huart)
{
    stop_rx(huart);
    return HAL_OK;
}

HAL_StatusTypeDef HAL_UART_Abort_IT(UART_HandleTypeDef *huart)
{
    (void)HAL_UART_Abort(huart);
    HAL_UART_AbortCpltCallback(huart);
    return HAL_OK;
}

HAL_StatusTypeDef HAL_UART_AbortTransmit_IT(UART_HandleTypeDef *huart)
{
    (void)HAL_UART_AbortTransmit(huart);
    HAL_UART_AbortTransmitCpltCallback(huart);
    return HAL_OK;
}

HAL_StatusTypeDef HAL_UART_AbortReceive_IT(UART_HandleTypeDef *huart)
{
    (void)HAL_UART_AbortReceive(huart);
    HAL_UART_AbortReceiveCpltCallback(huart);
    return HAL_OK;
}

void HAL_UART_IRQHandler(UART_HandleTypeDef *huart) {}

HAL_UART_StateTypeDef HAL_UART_GetState(const UART_HandleTypeDef *huart)
{
    return (HAL_UART_StateTypeDef)(huart->gState | huart->RxState);
}

uint32_t HAL_UART_GetError(const UART_HandleTypeDef *huart) { return huart->ErrorCode; }
