/*
 * libvmcu/shims/stm32-hal/include/stm32f4xx_hal_i2c.h — Module I2C du shim.
 *
 * Phase 1 : non simulé. Init appelle HAL_I2C_MspInit ; tout transfert se
 * comporte comme sur un bus sans composant : HAL_ERROR et ErrorCode =
 * HAL_I2C_ERROR_AF (adresse non acquittée). Simulation en phase 3.
 */
#ifndef PREBENCH_STM32F4XX_HAL_I2C_H
#define PREBENCH_STM32F4XX_HAL_I2C_H

#include "stm32f4xx_hal_dma.h"

#ifdef __cplusplus
extern "C" {
#endif

typedef struct {
    uint32_t ClockSpeed;
    uint32_t DutyCycle;
    uint32_t OwnAddress1;
    uint32_t AddressingMode;
    uint32_t DualAddressMode;
    uint32_t OwnAddress2;
    uint32_t GeneralCallMode;
    uint32_t NoStretchMode;
} I2C_InitTypeDef;

typedef enum {
    HAL_I2C_STATE_RESET          = 0x00U,
    HAL_I2C_STATE_READY          = 0x20U,
    HAL_I2C_STATE_BUSY           = 0x24U,
    HAL_I2C_STATE_BUSY_TX        = 0x21U,
    HAL_I2C_STATE_BUSY_RX        = 0x22U,
    HAL_I2C_STATE_LISTEN         = 0x28U,
    HAL_I2C_STATE_BUSY_TX_LISTEN = 0x29U,
    HAL_I2C_STATE_BUSY_RX_LISTEN = 0x2AU,
    HAL_I2C_STATE_ABORT          = 0x60U,
    HAL_I2C_STATE_TIMEOUT        = 0xA0U,
    HAL_I2C_STATE_ERROR          = 0xE0U
} HAL_I2C_StateTypeDef;

typedef enum {
    HAL_I2C_MODE_NONE   = 0x00U,
    HAL_I2C_MODE_MASTER = 0x10U,
    HAL_I2C_MODE_SLAVE  = 0x20U,
    HAL_I2C_MODE_MEM    = 0x40U
} HAL_I2C_ModeTypeDef;

typedef struct __I2C_HandleTypeDef {
    I2C_TypeDef               *Instance;
    I2C_InitTypeDef            Init;
    uint8_t                   *pBuffPtr;
    uint16_t                   XferSize;
    __IO uint16_t              XferCount;
    __IO uint32_t              XferOptions;
    __IO uint32_t              PreviousState;
    DMA_HandleTypeDef         *hdmatx;
    DMA_HandleTypeDef         *hdmarx;
    HAL_LockTypeDef            Lock;
    __IO HAL_I2C_StateTypeDef  State;
    __IO HAL_I2C_ModeTypeDef   Mode;
    __IO uint32_t              ErrorCode;
    __IO uint32_t              Devaddress;
    __IO uint32_t              Memaddress;
    __IO uint32_t              MemaddSize;
    __IO uint32_t              EventCount;
} I2C_HandleTypeDef;

#define HAL_I2C_ERROR_NONE    0x00000000U
#define HAL_I2C_ERROR_BERR    0x00000001U
#define HAL_I2C_ERROR_ARLO    0x00000002U
#define HAL_I2C_ERROR_AF      0x00000004U
#define HAL_I2C_ERROR_OVR     0x00000008U
#define HAL_I2C_ERROR_DMA     0x00000010U
#define HAL_I2C_ERROR_TIMEOUT 0x00000020U
#define HAL_I2C_ERROR_SIZE    0x00000040U

#define I2C_DUTYCYCLE_2    0x00000000U
#define I2C_DUTYCYCLE_16_9 0x00004000U
#define I2C_ADDRESSINGMODE_7BIT  0x00004000U
#define I2C_ADDRESSINGMODE_10BIT 0x0000C000U
#define I2C_DUALADDRESS_DISABLE 0x00000000U
#define I2C_DUALADDRESS_ENABLE  0x00000001U
#define I2C_GENERALCALL_DISABLE 0x00000000U
#define I2C_GENERALCALL_ENABLE  0x00000040U
#define I2C_NOSTRETCH_DISABLE 0x00000000U
#define I2C_NOSTRETCH_ENABLE  0x00000080U
#define I2C_MEMADD_SIZE_8BIT  0x00000001U
#define I2C_MEMADD_SIZE_16BIT 0x00000010U
#define I2C_DIRECTION_RECEIVE  0x00000000U
#define I2C_DIRECTION_TRANSMIT 0x00000001U
#define I2C_ANALOGFILTER_ENABLE  0x00000000U
#define I2C_ANALOGFILTER_DISABLE 0x00000010U
#define I2C_FIRST_FRAME          0x00000001U
#define I2C_FIRST_AND_NEXT_FRAME 0x00000002U
#define I2C_NEXT_FRAME           0x00000004U
#define I2C_FIRST_AND_LAST_FRAME 0x00000008U
#define I2C_LAST_FRAME_NO_STOP   0x00000010U
#define I2C_LAST_FRAME           0x00000020U

#define __HAL_I2C_ENABLE(__HANDLE__)  ((void)(__HANDLE__))
#define __HAL_I2C_DISABLE(__HANDLE__) ((void)(__HANDLE__))
#define __HAL_I2C_ENABLE_IT(__HANDLE__, __INTERRUPT__)  ((void)(__HANDLE__), (void)(__INTERRUPT__))
#define __HAL_I2C_DISABLE_IT(__HANDLE__, __INTERRUPT__) ((void)(__HANDLE__), (void)(__INTERRUPT__))
#define __HAL_I2C_GET_FLAG(__HANDLE__, __FLAG__)   ((void)(__HANDLE__), (void)(__FLAG__), RESET)
#define __HAL_I2C_CLEAR_FLAG(__HANDLE__, __FLAG__) ((void)(__HANDLE__), (void)(__FLAG__))

HAL_StatusTypeDef HAL_I2C_Init(I2C_HandleTypeDef *hi2c);
HAL_StatusTypeDef HAL_I2C_DeInit(I2C_HandleTypeDef *hi2c);
void HAL_I2C_MspInit(I2C_HandleTypeDef *hi2c);
void HAL_I2C_MspDeInit(I2C_HandleTypeDef *hi2c);

HAL_StatusTypeDef HAL_I2C_Master_Transmit(I2C_HandleTypeDef *hi2c, uint16_t DevAddress, uint8_t *pData, uint16_t Size, uint32_t Timeout);
HAL_StatusTypeDef HAL_I2C_Master_Receive(I2C_HandleTypeDef *hi2c, uint16_t DevAddress, uint8_t *pData, uint16_t Size, uint32_t Timeout);
HAL_StatusTypeDef HAL_I2C_Slave_Transmit(I2C_HandleTypeDef *hi2c, uint8_t *pData, uint16_t Size, uint32_t Timeout);
HAL_StatusTypeDef HAL_I2C_Slave_Receive(I2C_HandleTypeDef *hi2c, uint8_t *pData, uint16_t Size, uint32_t Timeout);
HAL_StatusTypeDef HAL_I2C_Mem_Write(I2C_HandleTypeDef *hi2c, uint16_t DevAddress, uint16_t MemAddress, uint16_t MemAddSize, uint8_t *pData, uint16_t Size, uint32_t Timeout);
HAL_StatusTypeDef HAL_I2C_Mem_Read(I2C_HandleTypeDef *hi2c, uint16_t DevAddress, uint16_t MemAddress, uint16_t MemAddSize, uint8_t *pData, uint16_t Size, uint32_t Timeout);
HAL_StatusTypeDef HAL_I2C_IsDeviceReady(I2C_HandleTypeDef *hi2c, uint16_t DevAddress, uint32_t Trials, uint32_t Timeout);

HAL_StatusTypeDef HAL_I2C_Master_Transmit_IT(I2C_HandleTypeDef *hi2c, uint16_t DevAddress, uint8_t *pData, uint16_t Size);
HAL_StatusTypeDef HAL_I2C_Master_Receive_IT(I2C_HandleTypeDef *hi2c, uint16_t DevAddress, uint8_t *pData, uint16_t Size);
HAL_StatusTypeDef HAL_I2C_Slave_Transmit_IT(I2C_HandleTypeDef *hi2c, uint8_t *pData, uint16_t Size);
HAL_StatusTypeDef HAL_I2C_Slave_Receive_IT(I2C_HandleTypeDef *hi2c, uint8_t *pData, uint16_t Size);
HAL_StatusTypeDef HAL_I2C_Mem_Write_IT(I2C_HandleTypeDef *hi2c, uint16_t DevAddress, uint16_t MemAddress, uint16_t MemAddSize, uint8_t *pData, uint16_t Size);
HAL_StatusTypeDef HAL_I2C_Mem_Read_IT(I2C_HandleTypeDef *hi2c, uint16_t DevAddress, uint16_t MemAddress, uint16_t MemAddSize, uint8_t *pData, uint16_t Size);
HAL_StatusTypeDef HAL_I2C_Master_Transmit_DMA(I2C_HandleTypeDef *hi2c, uint16_t DevAddress, uint8_t *pData, uint16_t Size);
HAL_StatusTypeDef HAL_I2C_Master_Receive_DMA(I2C_HandleTypeDef *hi2c, uint16_t DevAddress, uint8_t *pData, uint16_t Size);
HAL_StatusTypeDef HAL_I2C_Slave_Transmit_DMA(I2C_HandleTypeDef *hi2c, uint8_t *pData, uint16_t Size);
HAL_StatusTypeDef HAL_I2C_Slave_Receive_DMA(I2C_HandleTypeDef *hi2c, uint8_t *pData, uint16_t Size);
HAL_StatusTypeDef HAL_I2C_Mem_Write_DMA(I2C_HandleTypeDef *hi2c, uint16_t DevAddress, uint16_t MemAddress, uint16_t MemAddSize, uint8_t *pData, uint16_t Size);
HAL_StatusTypeDef HAL_I2C_Mem_Read_DMA(I2C_HandleTypeDef *hi2c, uint16_t DevAddress, uint16_t MemAddress, uint16_t MemAddSize, uint8_t *pData, uint16_t Size);
HAL_StatusTypeDef HAL_I2C_EnableListen_IT(I2C_HandleTypeDef *hi2c);
HAL_StatusTypeDef HAL_I2C_DisableListen_IT(I2C_HandleTypeDef *hi2c);
HAL_StatusTypeDef HAL_I2C_Master_Abort_IT(I2C_HandleTypeDef *hi2c, uint16_t DevAddress);

void HAL_I2C_EV_IRQHandler(I2C_HandleTypeDef *hi2c);
void HAL_I2C_ER_IRQHandler(I2C_HandleTypeDef *hi2c);
void HAL_I2C_MasterTxCpltCallback(I2C_HandleTypeDef *hi2c);
void HAL_I2C_MasterRxCpltCallback(I2C_HandleTypeDef *hi2c);
void HAL_I2C_SlaveTxCpltCallback(I2C_HandleTypeDef *hi2c);
void HAL_I2C_SlaveRxCpltCallback(I2C_HandleTypeDef *hi2c);
void HAL_I2C_AddrCallback(I2C_HandleTypeDef *hi2c, uint8_t TransferDirection, uint16_t AddrMatchCode);
void HAL_I2C_ListenCpltCallback(I2C_HandleTypeDef *hi2c);
void HAL_I2C_MemTxCpltCallback(I2C_HandleTypeDef *hi2c);
void HAL_I2C_MemRxCpltCallback(I2C_HandleTypeDef *hi2c);
void HAL_I2C_ErrorCallback(I2C_HandleTypeDef *hi2c);
void HAL_I2C_AbortCpltCallback(I2C_HandleTypeDef *hi2c);

HAL_I2C_StateTypeDef HAL_I2C_GetState(const I2C_HandleTypeDef *hi2c);
HAL_I2C_ModeTypeDef HAL_I2C_GetMode(const I2C_HandleTypeDef *hi2c);
uint32_t HAL_I2C_GetError(const I2C_HandleTypeDef *hi2c);

HAL_StatusTypeDef HAL_I2CEx_ConfigAnalogFilter(I2C_HandleTypeDef *hi2c, uint32_t AnalogFilter);
HAL_StatusTypeDef HAL_I2CEx_ConfigDigitalFilter(I2C_HandleTypeDef *hi2c, uint32_t DigitalFilter);

#ifdef __cplusplus
}
#endif

#endif /* PREBENCH_STM32F4XX_HAL_I2C_H */
