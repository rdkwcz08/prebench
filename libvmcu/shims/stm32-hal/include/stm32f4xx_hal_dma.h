/*
 * libvmcu/shims/stm32-hal/include/stm32f4xx_hal_dma.h — Module DMA du shim.
 *
 * Non simulé (DMA hors périmètre, section 3.3) : les fonctions compilent,
 * avertissent et n'effectuent aucun transfert. Les périphériques simulés
 * (UART, SPI…) traiteront leurs variantes _DMA comme des transferts entiers.
 */
#ifndef PREBENCH_STM32F4XX_HAL_DMA_H
#define PREBENCH_STM32F4XX_HAL_DMA_H

#include "stm32f4xx_hal_def.h"

#ifdef __cplusplus
extern "C" {
#endif

typedef struct {
    uint32_t Channel;
    uint32_t Direction;
    uint32_t PeriphInc;
    uint32_t MemInc;
    uint32_t PeriphDataAlignment;
    uint32_t MemDataAlignment;
    uint32_t Mode;
    uint32_t Priority;
    uint32_t FIFOMode;
    uint32_t FIFOThreshold;
    uint32_t MemBurst;
    uint32_t PeriphBurst;
} DMA_InitTypeDef;

typedef enum {
    HAL_DMA_STATE_RESET   = 0x00U,
    HAL_DMA_STATE_READY   = 0x01U,
    HAL_DMA_STATE_BUSY    = 0x02U,
    HAL_DMA_STATE_TIMEOUT = 0x03U,
    HAL_DMA_STATE_ERROR   = 0x04U,
    HAL_DMA_STATE_ABORT   = 0x05U
} HAL_DMA_StateTypeDef;

typedef enum {
    HAL_DMA_FULL_TRANSFER = 0x00U,
    HAL_DMA_HALF_TRANSFER = 0x01U
} HAL_DMA_LevelCompleteTypeDef;

typedef enum {
    HAL_DMA_XFER_CPLT_CB_ID       = 0x00U,
    HAL_DMA_XFER_HALFCPLT_CB_ID   = 0x01U,
    HAL_DMA_XFER_M1CPLT_CB_ID     = 0x02U,
    HAL_DMA_XFER_M1HALFCPLT_CB_ID = 0x03U,
    HAL_DMA_XFER_ERROR_CB_ID      = 0x04U,
    HAL_DMA_XFER_ABORT_CB_ID      = 0x05U,
    HAL_DMA_XFER_ALL_CB_ID        = 0x06U
} HAL_DMA_CallbackIDTypeDef;

typedef struct __DMA_HandleTypeDef {
    DMA_Stream_TypeDef        *Instance;
    DMA_InitTypeDef            Init;
    HAL_LockTypeDef            Lock;
    __IO HAL_DMA_StateTypeDef  State;
    void                      *Parent;
    void (*XferCpltCallback)(struct __DMA_HandleTypeDef *hdma);
    void (*XferHalfCpltCallback)(struct __DMA_HandleTypeDef *hdma);
    void (*XferM1CpltCallback)(struct __DMA_HandleTypeDef *hdma);
    void (*XferM1HalfCpltCallback)(struct __DMA_HandleTypeDef *hdma);
    void (*XferErrorCallback)(struct __DMA_HandleTypeDef *hdma);
    void (*XferAbortCallback)(struct __DMA_HandleTypeDef *hdma);
    __IO uint32_t              ErrorCode;
    uint32_t                   StreamBaseAddress;
    uint32_t                   StreamIndex;
} DMA_HandleTypeDef;

#define HAL_DMA_ERROR_NONE          0x00000000U
#define HAL_DMA_ERROR_TE            0x00000001U
#define HAL_DMA_ERROR_FE            0x00000002U
#define HAL_DMA_ERROR_DME           0x00000004U
#define HAL_DMA_ERROR_TIMEOUT       0x00000020U
#define HAL_DMA_ERROR_PARAM         0x00000040U
#define HAL_DMA_ERROR_NO_XFER       0x00000080U
#define HAL_DMA_ERROR_NOT_SUPPORTED 0x00000100U

#define DMA_CHANNEL_0 0x00000000U
#define DMA_CHANNEL_1 0x02000000U
#define DMA_CHANNEL_2 0x04000000U
#define DMA_CHANNEL_3 0x06000000U
#define DMA_CHANNEL_4 0x08000000U
#define DMA_CHANNEL_5 0x0A000000U
#define DMA_CHANNEL_6 0x0C000000U
#define DMA_CHANNEL_7 0x0E000000U

#define DMA_PERIPH_TO_MEMORY 0x00000000U
#define DMA_MEMORY_TO_PERIPH 0x00000040U
#define DMA_MEMORY_TO_MEMORY 0x00000080U
#define DMA_PINC_ENABLE      0x00000200U
#define DMA_PINC_DISABLE     0x00000000U
#define DMA_MINC_ENABLE      0x00000400U
#define DMA_MINC_DISABLE     0x00000000U
#define DMA_PDATAALIGN_BYTE     0x00000000U
#define DMA_PDATAALIGN_HALFWORD 0x00000800U
#define DMA_PDATAALIGN_WORD     0x00001000U
#define DMA_MDATAALIGN_BYTE     0x00000000U
#define DMA_MDATAALIGN_HALFWORD 0x00002000U
#define DMA_MDATAALIGN_WORD     0x00004000U
#define DMA_NORMAL   0x00000000U
#define DMA_CIRCULAR 0x00000100U
#define DMA_PFCTRL   0x00000020U
#define DMA_PRIORITY_LOW       0x00000000U
#define DMA_PRIORITY_MEDIUM    0x00010000U
#define DMA_PRIORITY_HIGH      0x00020000U
#define DMA_PRIORITY_VERY_HIGH 0x00030000U
#define DMA_FIFOMODE_DISABLE 0x00000000U
#define DMA_FIFOMODE_ENABLE  0x00000004U
#define DMA_FIFO_THRESHOLD_1QUARTERFULL  0x00000000U
#define DMA_FIFO_THRESHOLD_HALFFULL      0x00000001U
#define DMA_FIFO_THRESHOLD_3QUARTERSFULL 0x00000002U
#define DMA_FIFO_THRESHOLD_FULL          0x00000003U
#define DMA_MBURST_SINGLE 0x00000000U
#define DMA_MBURST_INC4   0x00800000U
#define DMA_MBURST_INC8   0x01000000U
#define DMA_MBURST_INC16  0x01800000U
#define DMA_PBURST_SINGLE 0x00000000U
#define DMA_PBURST_INC4   0x00200000U
#define DMA_PBURST_INC8   0x00400000U
#define DMA_PBURST_INC16  0x00600000U

#define DMA_IT_TC  0x00000010U
#define DMA_IT_HT  0x00000008U
#define DMA_IT_TE  0x00000004U
#define DMA_IT_DME 0x00000002U
#define DMA_IT_FE  0x00000080U

#define __HAL_DMA_ENABLE(__HANDLE__)                  ((void)(__HANDLE__))
#define __HAL_DMA_DISABLE(__HANDLE__)                 ((void)(__HANDLE__))
#define __HAL_DMA_ENABLE_IT(__HANDLE__, __INTERRUPT__)  ((void)(__HANDLE__), (void)(__INTERRUPT__))
#define __HAL_DMA_DISABLE_IT(__HANDLE__, __INTERRUPT__) ((void)(__HANDLE__), (void)(__INTERRUPT__))
#define __HAL_DMA_GET_COUNTER(__HANDLE__)             ((__HANDLE__)->Instance->NDTR)
#define __HAL_DMA_SET_COUNTER(__HANDLE__, __COUNTER__) ((__HANDLE__)->Instance->NDTR = (uint16_t)(__COUNTER__))

HAL_StatusTypeDef HAL_DMA_Init(DMA_HandleTypeDef *hdma);
HAL_StatusTypeDef HAL_DMA_DeInit(DMA_HandleTypeDef *hdma);
HAL_StatusTypeDef HAL_DMA_Start(DMA_HandleTypeDef *hdma, uint32_t SrcAddress, uint32_t DstAddress, uint32_t DataLength);
HAL_StatusTypeDef HAL_DMA_Start_IT(DMA_HandleTypeDef *hdma, uint32_t SrcAddress, uint32_t DstAddress, uint32_t DataLength);
HAL_StatusTypeDef HAL_DMA_Abort(DMA_HandleTypeDef *hdma);
HAL_StatusTypeDef HAL_DMA_Abort_IT(DMA_HandleTypeDef *hdma);
HAL_StatusTypeDef HAL_DMA_PollForTransfer(DMA_HandleTypeDef *hdma, HAL_DMA_LevelCompleteTypeDef CompleteLevel, uint32_t Timeout);
void HAL_DMA_IRQHandler(DMA_HandleTypeDef *hdma);
HAL_StatusTypeDef HAL_DMA_CleanCallbacks(DMA_HandleTypeDef *hdma);
HAL_StatusTypeDef HAL_DMA_RegisterCallback(DMA_HandleTypeDef *hdma, HAL_DMA_CallbackIDTypeDef CallbackID,
                                           void (*pCallback)(DMA_HandleTypeDef *_hdma));
HAL_StatusTypeDef HAL_DMA_UnRegisterCallback(DMA_HandleTypeDef *hdma, HAL_DMA_CallbackIDTypeDef CallbackID);
HAL_DMA_StateTypeDef HAL_DMA_GetState(DMA_HandleTypeDef *hdma);
uint32_t HAL_DMA_GetError(DMA_HandleTypeDef *hdma);

#ifdef __cplusplus
}
#endif

#endif /* PREBENCH_STM32F4XX_HAL_DMA_H */
