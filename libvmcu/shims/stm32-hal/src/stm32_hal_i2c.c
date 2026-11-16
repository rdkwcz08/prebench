/*
 * libvmcu/shims/stm32-hal/src/stm32_hal_i2c.c — I2C maître du HAL STM32 sur
 * libvmcu (vmcu_i2c_*) ; phase 3, étape 2.
 *
 * Bus : numéro du périphérique (I2C1 -> 1), dérivé de hi2c->Instance.
 * Adresse : DevAddress est l'adresse 7 bits décalée d'un bit, comme dans le HAL.
 * - Transferts bloquants : une transaction synchrone (le firmware attend la
 *   réponse du modèle). NACK : HAL_ERROR et ErrorCode = HAL_I2C_ERROR_AF.
 * - Mem_Write / Mem_Read : adresse mémoire (1 ou 2 octets, poids fort d'abord)
 *   puis données, en une transaction (start répété pour la lecture).
 * - Variantes _IT et _DMA : transaction immédiate, puis callback de fin
 *   (MasterTx/Rx, MemTx/Rx, ou ErrorCallback sur NACK) depuis le fil IRQ
 *   (vmcu_irq_post), comme l'UART. Un callback peut relancer un transfert.
 * - IsDeviceReady : sonde d'adresse (transaction vide), Trials fois au plus.
 * Limites : pas de mode esclave ni d'adressage 10 bits, pas de transferts
 * séquentiels ; transaction de 4096 octets au plus (VMCU_I2C_MAX_XFER), au-delà
 * HAL_ERROR et HAL_I2C_ERROR_SIZE. Ces cas avertissent « non simulé ».
 */
#include "stm32_hal_internal.h"

#include <stddef.h>
#include <string.h>

#if defined(__GNUC__) || defined(__clang__)
#pragma GCC diagnostic ignored "-Wunused-parameter"
#endif

#define I2C_COUNT 4

/** Numéro du bus, ou -1 si le handle ne désigne pas un I2C connu. */
static int i2c_bus(const I2C_HandleTypeDef *hi2c)
{
    if (hi2c == NULL || hi2c->Instance == NULL) {
        return -1;
    }
    ptrdiff_t i = hi2c->Instance - prebench_i2c;
    return (i > 0 && i < I2C_COUNT) ? (int)i : -1;
}

HAL_StatusTypeDef HAL_I2C_Init(I2C_HandleTypeDef *hi2c)
{
    int bus = i2c_bus(hi2c);
    if (bus < 0) {
        return HAL_ERROR;
    }
    if (hi2c->State == HAL_I2C_STATE_RESET) {
        hi2c->Lock = HAL_UNLOCKED;
        HAL_I2C_MspInit(hi2c);
    }
    if (hi2c->Init.AddressingMode == I2C_ADDRESSINGMODE_10BIT) {
        vmcu_warn_unsimulated("HAL_I2C_Init (adressage 10 bits)");
    }
    (void)vmcu_i2c_config((uint8_t)bus, hi2c->Init.ClockSpeed);
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
    hi2c->Mode = HAL_I2C_MODE_NONE;
    return HAL_OK;
}

/* --- Transaction ------------------------------------------------------------------------ */

/** Résultat HAL d'une transaction, ErrorCode renseigné. */
static HAL_StatusTypeDef to_hal(I2C_HandleTypeDef *hi2c, vmcu_status_t st)
{
    switch (st) {
    case VMCU_OK:
        hi2c->ErrorCode = HAL_I2C_ERROR_NONE;
        return HAL_OK;
    case VMCU_ERR_NACK:
        hi2c->ErrorCode = HAL_I2C_ERROR_AF;
        return HAL_ERROR;
    case VMCU_ERR_TIMEOUT:
        hi2c->ErrorCode = HAL_I2C_ERROR_TIMEOUT;
        return HAL_TIMEOUT;
    case VMCU_ERR_INVALID_ARG:
        hi2c->ErrorCode = HAL_I2C_ERROR_SIZE;
        return HAL_ERROR;
    default:
        hi2c->ErrorCode = HAL_I2C_ERROR_BERR;
        return HAL_ERROR;
    }
}

/**
 * Une transaction : écrit [mem (mem_size octets)] + tx, puis lit rx.
 * mem_size : 0 (transfert maître simple), 1 ou 2.
 */
static vmcu_status_t xfer(I2C_HandleTypeDef *hi2c, uint16_t dev, uint16_t mem, uint8_t mem_size,
                          const uint8_t *tx, uint16_t txlen, uint8_t *rx, uint16_t rxlen)
{
    int bus = i2c_bus(hi2c);
    if (bus < 0) {
        return VMCU_ERR_INVALID_ARG;
    }
    if (hi2c->Init.AddressingMode == I2C_ADDRESSINGMODE_10BIT) {
        return VMCU_ERR_UNSUPPORTED;
    }
    if ((size_t)txlen + mem_size > VMCU_I2C_MAX_XFER || rxlen > VMCU_I2C_MAX_XFER) {
        vmcu_warn_unsimulated("transfert I2C de plus de 4096 octets");
        return VMCU_ERR_INVALID_ARG;
    }
    uint8_t addr7 = (uint8_t)((dev >> 1) & 0x7F);
    if (mem_size == 0) {
        return vmcu_i2c_xfer((uint8_t)bus, addr7, tx, txlen, rx, rxlen, 0);
    }
    uint8_t buf[2 + VMCU_I2C_MAX_XFER];
    size_t n = 0;
    if (mem_size == 2) {
        buf[n++] = (uint8_t)(mem >> 8);
    }
    buf[n++] = (uint8_t)(mem & 0xFF);
    if (txlen > 0) {
        memcpy(buf + n, tx, txlen);
        n += txlen;
    }
    return vmcu_i2c_xfer((uint8_t)bus, addr7, buf, n, rx, rxlen, 0);
}

static uint8_t mem_bytes(uint16_t MemAddSize)
{
    return (MemAddSize == I2C_MEMADD_SIZE_16BIT) ? 2U : 1U;
}

/* --- Transferts bloquants --------------------------------------------------------------- */

static HAL_StatusTypeDef blocking(I2C_HandleTypeDef *hi2c, HAL_I2C_ModeTypeDef mode, uint16_t dev,
                                  uint16_t mem, uint8_t mem_size, const uint8_t *tx,
                                  uint16_t txlen, uint8_t *rx, uint16_t rxlen)
{
    if (i2c_bus(hi2c) < 0 || ((tx == NULL && txlen > 0) || (rx == NULL && rxlen > 0))) {
        return HAL_ERROR;
    }
    if (hi2c->State != HAL_I2C_STATE_READY) {
        return HAL_BUSY;
    }
    hi2c->Mode = mode;
    hi2c->Devaddress = dev;
    vmcu_status_t st = xfer(hi2c, dev, mem, mem_size, tx, txlen, rx, rxlen);
    hi2c->Mode = HAL_I2C_MODE_NONE;
    if (st == VMCU_ERR_UNSUPPORTED) {
        vmcu_warn_unsimulated("HAL_I2C (adressage 10 bits)");
    }
    return to_hal(hi2c, st);
}

HAL_StatusTypeDef HAL_I2C_Master_Transmit(I2C_HandleTypeDef *hi2c, uint16_t DevAddress, uint8_t *pData, uint16_t Size, uint32_t Timeout)
{
    return blocking(hi2c, HAL_I2C_MODE_MASTER, DevAddress, 0, 0, pData, Size, NULL, 0);
}

HAL_StatusTypeDef HAL_I2C_Master_Receive(I2C_HandleTypeDef *hi2c, uint16_t DevAddress, uint8_t *pData, uint16_t Size, uint32_t Timeout)
{
    return blocking(hi2c, HAL_I2C_MODE_MASTER, DevAddress, 0, 0, NULL, 0, pData, Size);
}

HAL_StatusTypeDef HAL_I2C_Mem_Write(I2C_HandleTypeDef *hi2c, uint16_t DevAddress, uint16_t MemAddress, uint16_t MemAddSize, uint8_t *pData, uint16_t Size, uint32_t Timeout)
{
    return blocking(hi2c, HAL_I2C_MODE_MEM, DevAddress, MemAddress, mem_bytes(MemAddSize), pData, Size, NULL, 0);
}

HAL_StatusTypeDef HAL_I2C_Mem_Read(I2C_HandleTypeDef *hi2c, uint16_t DevAddress, uint16_t MemAddress, uint16_t MemAddSize, uint8_t *pData, uint16_t Size, uint32_t Timeout)
{
    return blocking(hi2c, HAL_I2C_MODE_MEM, DevAddress, MemAddress, mem_bytes(MemAddSize), NULL, 0, pData, Size);
}

HAL_StatusTypeDef HAL_I2C_IsDeviceReady(I2C_HandleTypeDef *hi2c, uint16_t DevAddress, uint32_t Trials, uint32_t Timeout)
{
    if (i2c_bus(hi2c) < 0) {
        return HAL_ERROR;
    }
    if (hi2c->State != HAL_I2C_STATE_READY) {
        return HAL_BUSY;
    }
    vmcu_status_t st = VMCU_ERR_NACK;
    for (uint32_t i = 0; i < (Trials == 0 ? 1U : Trials); ++i) {
        st = xfer(hi2c, DevAddress, 0, 0, NULL, 0, NULL, 0);
        if (st != VMCU_ERR_NACK) {
            break; /* acquitté, ou erreur qui ne changera pas en réessayant */
        }
    }
    /* Comme le HAL : HAL_ERROR sans ErrorCode quand personne ne répond. */
    HAL_StatusTypeDef hal = to_hal(hi2c, st);
    if (st == VMCU_ERR_NACK) {
        hi2c->ErrorCode = HAL_I2C_ERROR_NONE;
    }
    return hal;
}

/* --- Variantes _IT et _DMA : transaction immédiate, callback depuis le fil IRQ -------- */

/* Fil IRQ : fin d'un transfert. State et Mode disent quel callback appeler. */
static void xfer_done(void *arg)
{
    I2C_HandleTypeDef *hi2c = (I2C_HandleTypeDef *)arg;
    HAL_I2C_StateTypeDef state = hi2c->State;
    HAL_I2C_ModeTypeDef mode = hi2c->Mode;
    hi2c->XferCount = 0;
    hi2c->State = HAL_I2C_STATE_READY;
    hi2c->Mode = HAL_I2C_MODE_NONE;
    if (hi2c->ErrorCode != HAL_I2C_ERROR_NONE) {
        HAL_I2C_ErrorCallback(hi2c);
    } else if (mode == HAL_I2C_MODE_MEM) {
        if (state == HAL_I2C_STATE_BUSY_RX) {
            HAL_I2C_MemRxCpltCallback(hi2c);
        } else {
            HAL_I2C_MemTxCpltCallback(hi2c);
        }
    } else if (state == HAL_I2C_STATE_BUSY_RX) {
        HAL_I2C_MasterRxCpltCallback(hi2c);
    } else {
        HAL_I2C_MasterTxCpltCallback(hi2c);
    }
}

static HAL_StatusTypeDef async(I2C_HandleTypeDef *hi2c, HAL_I2C_ModeTypeDef mode, uint16_t dev,
                               uint16_t mem, uint8_t mem_size, uint8_t *data, uint16_t size,
                               bool receive)
{
    if (i2c_bus(hi2c) < 0 || data == NULL || size == 0) {
        return HAL_ERROR;
    }
    if (hi2c->State != HAL_I2C_STATE_READY) {
        return HAL_BUSY;
    }
    hi2c->State = receive ? HAL_I2C_STATE_BUSY_RX : HAL_I2C_STATE_BUSY_TX;
    hi2c->Mode = mode;
    hi2c->Devaddress = dev;
    hi2c->Memaddress = mem;
    hi2c->MemaddSize = mem_size;
    hi2c->pBuffPtr = data;
    hi2c->XferSize = size;
    hi2c->XferCount = size;
    vmcu_status_t st = receive ? xfer(hi2c, dev, mem, mem_size, NULL, 0, data, size)
                               : xfer(hi2c, dev, mem, mem_size, data, size, NULL, 0);
    if (st == VMCU_ERR_UNSUPPORTED) {
        vmcu_warn_unsimulated("HAL_I2C (adressage 10 bits)");
    }
    if (st == VMCU_ERR_INVALID_ARG) {
        /* Refusé avant tout échange, comme un argument invalide du HAL. */
        hi2c->State = HAL_I2C_STATE_READY;
        hi2c->Mode = HAL_I2C_MODE_NONE;
        (void)to_hal(hi2c, st);
        return HAL_ERROR;
    }
    (void)to_hal(hi2c, st); /* NACK ou délai : remonté par HAL_I2C_ErrorCallback */
    if (vmcu_irq_post(xfer_done, hi2c) != VMCU_OK) {
        hi2c->State = HAL_I2C_STATE_READY;
        hi2c->Mode = HAL_I2C_MODE_NONE;
        return HAL_BUSY;
    }
    return HAL_OK;
}

HAL_StatusTypeDef HAL_I2C_Master_Transmit_IT(I2C_HandleTypeDef *hi2c, uint16_t DevAddress, uint8_t *pData, uint16_t Size)
{
    return async(hi2c, HAL_I2C_MODE_MASTER, DevAddress, 0, 0, pData, Size, false);
}

HAL_StatusTypeDef HAL_I2C_Master_Receive_IT(I2C_HandleTypeDef *hi2c, uint16_t DevAddress, uint8_t *pData, uint16_t Size)
{
    return async(hi2c, HAL_I2C_MODE_MASTER, DevAddress, 0, 0, pData, Size, true);
}

HAL_StatusTypeDef HAL_I2C_Mem_Write_IT(I2C_HandleTypeDef *hi2c, uint16_t DevAddress, uint16_t MemAddress, uint16_t MemAddSize, uint8_t *pData, uint16_t Size)
{
    return async(hi2c, HAL_I2C_MODE_MEM, DevAddress, MemAddress, mem_bytes(MemAddSize), pData, Size, false);
}

HAL_StatusTypeDef HAL_I2C_Mem_Read_IT(I2C_HandleTypeDef *hi2c, uint16_t DevAddress, uint16_t MemAddress, uint16_t MemAddSize, uint8_t *pData, uint16_t Size)
{
    return async(hi2c, HAL_I2C_MODE_MEM, DevAddress, MemAddress, mem_bytes(MemAddSize), pData, Size, true);
}

HAL_StatusTypeDef HAL_I2C_Master_Transmit_DMA(I2C_HandleTypeDef *hi2c, uint16_t DevAddress, uint8_t *pData, uint16_t Size)
{
    return async(hi2c, HAL_I2C_MODE_MASTER, DevAddress, 0, 0, pData, Size, false);
}

HAL_StatusTypeDef HAL_I2C_Master_Receive_DMA(I2C_HandleTypeDef *hi2c, uint16_t DevAddress, uint8_t *pData, uint16_t Size)
{
    return async(hi2c, HAL_I2C_MODE_MASTER, DevAddress, 0, 0, pData, Size, true);
}

HAL_StatusTypeDef HAL_I2C_Mem_Write_DMA(I2C_HandleTypeDef *hi2c, uint16_t DevAddress, uint16_t MemAddress, uint16_t MemAddSize, uint8_t *pData, uint16_t Size)
{
    return async(hi2c, HAL_I2C_MODE_MEM, DevAddress, MemAddress, mem_bytes(MemAddSize), pData, Size, false);
}

HAL_StatusTypeDef HAL_I2C_Mem_Read_DMA(I2C_HandleTypeDef *hi2c, uint16_t DevAddress, uint16_t MemAddress, uint16_t MemAddSize, uint8_t *pData, uint16_t Size)
{
    return async(hi2c, HAL_I2C_MODE_MEM, DevAddress, MemAddress, mem_bytes(MemAddSize), pData, Size, true);
}

/* --- Mode esclave : non simulé (bus vu sans maître) ------------------------------------- */

static HAL_StatusTypeDef slave_unsimulated(I2C_HandleTypeDef *hi2c, const char *name)
{
    vmcu_warn_unsimulated(name);
    if (hi2c != NULL) {
        hi2c->ErrorCode = HAL_I2C_ERROR_TIMEOUT;
    }
    return HAL_ERROR;
}

HAL_StatusTypeDef HAL_I2C_Slave_Transmit(I2C_HandleTypeDef *hi2c, uint8_t *pData, uint16_t Size, uint32_t Timeout) { return slave_unsimulated(hi2c, __func__); }
HAL_StatusTypeDef HAL_I2C_Slave_Receive(I2C_HandleTypeDef *hi2c, uint8_t *pData, uint16_t Size, uint32_t Timeout) { return slave_unsimulated(hi2c, __func__); }
HAL_StatusTypeDef HAL_I2C_Slave_Transmit_IT(I2C_HandleTypeDef *hi2c, uint8_t *pData, uint16_t Size) { return slave_unsimulated(hi2c, __func__); }
HAL_StatusTypeDef HAL_I2C_Slave_Receive_IT(I2C_HandleTypeDef *hi2c, uint8_t *pData, uint16_t Size) { return slave_unsimulated(hi2c, __func__); }
HAL_StatusTypeDef HAL_I2C_Slave_Transmit_DMA(I2C_HandleTypeDef *hi2c, uint8_t *pData, uint16_t Size) { return slave_unsimulated(hi2c, __func__); }
HAL_StatusTypeDef HAL_I2C_Slave_Receive_DMA(I2C_HandleTypeDef *hi2c, uint8_t *pData, uint16_t Size) { return slave_unsimulated(hi2c, __func__); }
HAL_StatusTypeDef HAL_I2C_EnableListen_IT(I2C_HandleTypeDef *hi2c) { PREBENCH_UNSIMULATED(); return HAL_OK; }
HAL_StatusTypeDef HAL_I2C_DisableListen_IT(I2C_HandleTypeDef *hi2c) { return HAL_OK; }

/* --- Divers ------------------------------------------------------------------------------ */

HAL_StatusTypeDef HAL_I2C_Master_Abort_IT(I2C_HandleTypeDef *hi2c, uint16_t DevAddress)
{
    return HAL_OK; /* les transferts sont déjà terminés quand l'appel rend la main */
}

void HAL_I2C_EV_IRQHandler(I2C_HandleTypeDef *hi2c) {}
void HAL_I2C_ER_IRQHandler(I2C_HandleTypeDef *hi2c) {}
HAL_I2C_StateTypeDef HAL_I2C_GetState(const I2C_HandleTypeDef *hi2c) { return hi2c->State; }
HAL_I2C_ModeTypeDef HAL_I2C_GetMode(const I2C_HandleTypeDef *hi2c) { return hi2c->Mode; }
uint32_t HAL_I2C_GetError(const I2C_HandleTypeDef *hi2c) { return hi2c->ErrorCode; }
HAL_StatusTypeDef HAL_I2CEx_ConfigAnalogFilter(I2C_HandleTypeDef *hi2c, uint32_t AnalogFilter) { return HAL_OK; }
HAL_StatusTypeDef HAL_I2CEx_ConfigDigitalFilter(I2C_HandleTypeDef *hi2c, uint32_t DigitalFilter) { return HAL_OK; }
