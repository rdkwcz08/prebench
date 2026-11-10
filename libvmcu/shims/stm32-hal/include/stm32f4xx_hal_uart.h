/*
 * libvmcu/shims/stm32-hal/include/stm32f4xx_hal_uart.h — Module UART du shim.
 *
 * Phase 1 : non simulé. Init appelle HAL_UART_MspInit (configuration des
 * broches, comme le vrai HAL) ; les émissions sont acceptées sans effet, les
 * réceptions bloquantes expirent. Simulation (moniteur série) en phase 2.
 */
#ifndef PREBENCH_STM32F4XX_HAL_UART_H
#define PREBENCH_STM32F4XX_HAL_UART_H

#include "stm32f4xx_hal_dma.h"

#ifdef __cplusplus
extern "C" {
#endif

typedef struct {
    uint32_t BaudRate;
    uint32_t WordLength;
    uint32_t StopBits;
    uint32_t Parity;
    uint32_t Mode;
    uint32_t HwFlowCtl;
    uint32_t OverSampling;
} UART_InitTypeDef;

typedef enum {
    HAL_UART_STATE_RESET      = 0x00U,
    HAL_UART_STATE_READY      = 0x20U,
    HAL_UART_STATE_BUSY       = 0x24U,
    HAL_UART_STATE_BUSY_TX    = 0x21U,
    HAL_UART_STATE_BUSY_RX    = 0x22U,
    HAL_UART_STATE_BUSY_TX_RX = 0x23U,
    HAL_UART_STATE_TIMEOUT    = 0xA0U,
    HAL_UART_STATE_ERROR      = 0xE0U
} HAL_UART_StateTypeDef;

typedef uint32_t HAL_UART_RxTypeTypeDef;
typedef uint32_t HAL_UART_RxEventTypeTypeDef;

typedef struct __UART_HandleTypeDef {
    USART_TypeDef                    *Instance;
    UART_InitTypeDef                  Init;
    const uint8_t                    *pTxBuffPtr;
    uint16_t                          TxXferSize;
    __IO uint16_t                     TxXferCount;
    uint8_t                          *pRxBuffPtr;
    uint16_t                          RxXferSize;
    __IO uint16_t                     RxXferCount;
    __IO HAL_UART_RxTypeTypeDef       ReceptionType;
    __IO HAL_UART_RxEventTypeTypeDef  RxEventType;
    DMA_HandleTypeDef                *hdmatx;
    DMA_HandleTypeDef                *hdmarx;
    HAL_LockTypeDef                   Lock;
    __IO HAL_UART_StateTypeDef        gState;
    __IO HAL_UART_StateTypeDef        RxState;
    __IO uint32_t                     ErrorCode;
} UART_HandleTypeDef;

#define HAL_UART_ERROR_NONE 0x00000000U
#define HAL_UART_ERROR_PE   0x00000001U
#define HAL_UART_ERROR_NE   0x00000002U
#define HAL_UART_ERROR_FE   0x00000004U
#define HAL_UART_ERROR_ORE  0x00000008U
#define HAL_UART_ERROR_DMA  0x00000010U

#define UART_WORDLENGTH_8B 0x00000000U
#define UART_WORDLENGTH_9B 0x00001000U
#define UART_STOPBITS_1    0x00000000U
#define UART_STOPBITS_2    0x00002000U
#define UART_PARITY_NONE   0x00000000U
#define UART_PARITY_EVEN   0x00000400U
#define UART_PARITY_ODD    0x00000600U
#define UART_HWCONTROL_NONE    0x00000000U
#define UART_HWCONTROL_RTS     0x00000100U
#define UART_HWCONTROL_CTS     0x00000200U
#define UART_HWCONTROL_RTS_CTS 0x00000300U
#define UART_MODE_RX    0x00000004U
#define UART_MODE_TX    0x00000008U
#define UART_MODE_TX_RX 0x0000000CU
#define UART_OVERSAMPLING_16 0x00000000U
#define UART_OVERSAMPLING_8  0x00008000U
#define UART_WAKEUPMETHOD_IDLELINE    0x00000000U
#define UART_WAKEUPMETHOD_ADDRESSMARK 0x00000800U
#define UART_LINBREAKDETECTLENGTH_10B 0x00000000U
#define UART_LINBREAKDETECTLENGTH_11B 0x00000020U

#define HAL_UART_RECEPTION_STANDARD 0x00000000U
#define HAL_UART_RECEPTION_TOIDLE   0x00000001U
#define HAL_UART_RXEVENT_TC   0x00000000U
#define HAL_UART_RXEVENT_HT   0x00000001U
#define HAL_UART_RXEVENT_IDLE 0x00000002U

#define UART_FLAG_CTS  0x00000200U
#define UART_FLAG_LBD  0x00000100U
#define UART_FLAG_TXE  0x00000080U
#define UART_FLAG_TC   0x00000040U
#define UART_FLAG_RXNE 0x00000020U
#define UART_FLAG_IDLE 0x00000010U
#define UART_FLAG_ORE  0x00000008U
#define UART_FLAG_NE   0x00000004U
#define UART_FLAG_FE   0x00000002U
#define UART_FLAG_PE   0x00000001U

#define UART_IT_PE   0x10000100U
#define UART_IT_TXE  0x10000080U
#define UART_IT_TC   0x10000040U
#define UART_IT_RXNE 0x10000020U
#define UART_IT_IDLE 0x10000010U
#define UART_IT_LBD  0x20000040U
#define UART_IT_CTS  0x30000400U
#define UART_IT_ERR  0x30000001U

/* Émetteur toujours prêt (TXE, TC) ; aucune donnée reçue en phase 1. */
FlagStatus prebench_uart_get_flag(const UART_HandleTypeDef *huart, uint32_t flag);
#define __HAL_UART_GET_FLAG(__HANDLE__, __FLAG__)   prebench_uart_get_flag((__HANDLE__), (__FLAG__))
#define __HAL_UART_CLEAR_FLAG(__HANDLE__, __FLAG__) ((void)(__HANDLE__), (void)(__FLAG__))
#define __HAL_UART_CLEAR_PEFLAG(__HANDLE__)   ((void)(__HANDLE__))
#define __HAL_UART_CLEAR_FEFLAG(__HANDLE__)   ((void)(__HANDLE__))
#define __HAL_UART_CLEAR_NEFLAG(__HANDLE__)   ((void)(__HANDLE__))
#define __HAL_UART_CLEAR_OREFLAG(__HANDLE__)  ((void)(__HANDLE__))
#define __HAL_UART_CLEAR_IDLEFLAG(__HANDLE__) ((void)(__HANDLE__))
#define __HAL_UART_ENABLE_IT(__HANDLE__, __INTERRUPT__)  ((void)(__HANDLE__), (void)(__INTERRUPT__))
#define __HAL_UART_DISABLE_IT(__HANDLE__, __INTERRUPT__) ((void)(__HANDLE__), (void)(__INTERRUPT__))
#define __HAL_UART_GET_IT_SOURCE(__HANDLE__, __IT__)     ((void)(__HANDLE__), (void)(__IT__), RESET)
#define __HAL_UART_ENABLE(__HANDLE__)  ((void)(__HANDLE__))
#define __HAL_UART_DISABLE(__HANDLE__) ((void)(__HANDLE__))

HAL_StatusTypeDef HAL_UART_Init(UART_HandleTypeDef *huart);
HAL_StatusTypeDef HAL_HalfDuplex_Init(UART_HandleTypeDef *huart);
HAL_StatusTypeDef HAL_LIN_Init(UART_HandleTypeDef *huart, uint32_t BreakDetectLength);
HAL_StatusTypeDef HAL_MultiProcessor_Init(UART_HandleTypeDef *huart, uint8_t Address, uint32_t WakeUpMethod);
HAL_StatusTypeDef HAL_UART_DeInit(UART_HandleTypeDef *huart);
void HAL_UART_MspInit(UART_HandleTypeDef *huart);
void HAL_UART_MspDeInit(UART_HandleTypeDef *huart);

HAL_StatusTypeDef HAL_UART_Transmit(UART_HandleTypeDef *huart, const uint8_t *pData, uint16_t Size, uint32_t Timeout);
HAL_StatusTypeDef HAL_UART_Receive(UART_HandleTypeDef *huart, uint8_t *pData, uint16_t Size, uint32_t Timeout);
HAL_StatusTypeDef HAL_UART_Transmit_IT(UART_HandleTypeDef *huart, const uint8_t *pData, uint16_t Size);
HAL_StatusTypeDef HAL_UART_Receive_IT(UART_HandleTypeDef *huart, uint8_t *pData, uint16_t Size);
HAL_StatusTypeDef HAL_UART_Transmit_DMA(UART_HandleTypeDef *huart, const uint8_t *pData, uint16_t Size);
HAL_StatusTypeDef HAL_UART_Receive_DMA(UART_HandleTypeDef *huart, uint8_t *pData, uint16_t Size);
HAL_StatusTypeDef HAL_UART_DMAPause(UART_HandleTypeDef *huart);
HAL_StatusTypeDef HAL_UART_DMAResume(UART_HandleTypeDef *huart);
HAL_StatusTypeDef HAL_UART_DMAStop(UART_HandleTypeDef *huart);
HAL_StatusTypeDef HAL_UARTEx_ReceiveToIdle(UART_HandleTypeDef *huart, uint8_t *pData, uint16_t Size, uint16_t *RxLen, uint32_t Timeout);
HAL_StatusTypeDef HAL_UARTEx_ReceiveToIdle_IT(UART_HandleTypeDef *huart, uint8_t *pData, uint16_t Size);
HAL_StatusTypeDef HAL_UARTEx_ReceiveToIdle_DMA(UART_HandleTypeDef *huart, uint8_t *pData, uint16_t Size);
HAL_UART_RxEventTypeTypeDef HAL_UARTEx_GetRxEventType(UART_HandleTypeDef *huart);
HAL_StatusTypeDef HAL_UART_Abort(UART_HandleTypeDef *huart);
HAL_StatusTypeDef HAL_UART_AbortTransmit(UART_HandleTypeDef *huart);
HAL_StatusTypeDef HAL_UART_AbortReceive(UART_HandleTypeDef *huart);
HAL_StatusTypeDef HAL_UART_Abort_IT(UART_HandleTypeDef *huart);
HAL_StatusTypeDef HAL_UART_AbortTransmit_IT(UART_HandleTypeDef *huart);
HAL_StatusTypeDef HAL_UART_AbortReceive_IT(UART_HandleTypeDef *huart);
void HAL_UART_IRQHandler(UART_HandleTypeDef *huart);

void HAL_UART_TxCpltCallback(UART_HandleTypeDef *huart);
void HAL_UART_TxHalfCpltCallback(UART_HandleTypeDef *huart);
void HAL_UART_RxCpltCallback(UART_HandleTypeDef *huart);
void HAL_UART_RxHalfCpltCallback(UART_HandleTypeDef *huart);
void HAL_UART_ErrorCallback(UART_HandleTypeDef *huart);
void HAL_UART_AbortCpltCallback(UART_HandleTypeDef *huart);
void HAL_UART_AbortTransmitCpltCallback(UART_HandleTypeDef *huart);
void HAL_UART_AbortReceiveCpltCallback(UART_HandleTypeDef *huart);
void HAL_UARTEx_RxEventCallback(UART_HandleTypeDef *huart, uint16_t Size);

HAL_UART_StateTypeDef HAL_UART_GetState(const UART_HandleTypeDef *huart);
uint32_t HAL_UART_GetError(const UART_HandleTypeDef *huart);

#ifdef __cplusplus
}
#endif

#endif /* PREBENCH_STM32F4XX_HAL_UART_H */
