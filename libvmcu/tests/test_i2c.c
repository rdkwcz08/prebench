/*
 * libvmcu/tests/test_i2c.c — I2C du HAL STM32 sur libvmcu, face à une fausse app
 * qui simule un esclave à registres en 0x50 (phase 3, étape 2) :
 * I2C_CONFIG, IsDeviceReady, Mem_Write et Mem_Read (adresse sur 8 et 16 bits),
 * transfert maître, NACK d'une adresse absente (ErrorCode AF), Mem_Read_IT et
 * Master_Transmit_DMA avec leur callback de fin, ErrorCallback sur NACK en _IT,
 * HAL_BUSY pendant un transfert en cours.
 */
#include <stdatomic.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include "fake_app.h"
#include "stm32f4xx_hal.h"
#include "vmcu_platform.h"
#include "vmcu_wire.h"

#define DEV (0x50u << 1)

static I2C_HandleTypeDef hi2c1;
static atomic_int mem_rx_done;
static atomic_int master_tx_done;
static atomic_int errors;

void HAL_I2C_MemRxCpltCallback(I2C_HandleTypeDef *hi2c)
{
    if (hi2c == &hi2c1) {
        atomic_fetch_add(&mem_rx_done, 1);
    }
}

void HAL_I2C_MasterTxCpltCallback(I2C_HandleTypeDef *hi2c)
{
    if (hi2c == &hi2c1) {
        atomic_fetch_add(&master_tx_done, 1);
    }
}

void HAL_I2C_ErrorCallback(I2C_HandleTypeDef *hi2c)
{
    if (hi2c == &hi2c1) {
        atomic_fetch_add(&errors, 1);
    }
}

static bool wait_for(atomic_int *v, int expected)
{
    for (int t = 0; t < 200; ++t) {
        if (atomic_load(v) >= expected) {
            return true;
        }
        vmcu_plat_sleep_ns(5000000ull);
    }
    return false;
}

int main(void)
{
    fake_app_t *fa = fake_app_create(1);
    fake_app_set_i2c_device(fa, 0x50);
    fake_app_set_i2c_reg(fa, 0x75, 0x68);
    fake_app_run(fa);
    CHECK(HAL_Init() == HAL_OK);

    /* I2C_CONFIG : bus 1, 400 kHz. */
    hi2c1.Instance = I2C1;
    hi2c1.Init.ClockSpeed = 400000;
    hi2c1.Init.AddressingMode = I2C_ADDRESSINGMODE_7BIT;
    CHECK(HAL_I2C_Init(&hi2c1) == HAL_OK);
    fake_frame_t f;
    CHECK(fake_app_wait(fa, VMCU_MSG_I2C_CONFIG, -1, -1, 1, 1000, &f));
    CHECK(f.data[0] == 1 && vmcu_wire_get_u32(f.data + 4) == 400000u);

    /* Sonde d'adresse : présent en 0x50, absent en 0x51. */
    CHECK(HAL_I2C_IsDeviceReady(&hi2c1, DEV, 3, 10) == HAL_OK);
    CHECK(HAL_I2C_IsDeviceReady(&hi2c1, 0x51u << 1, 3, 10) == HAL_ERROR);

    /* Registre lu tel que posé par la fausse app ; trame : adresse 7 bits, 1 octet écrit, 1 lu. */
    uint8_t v = 0;
    CHECK(HAL_I2C_Mem_Read(&hi2c1, DEV, 0x75, I2C_MEMADD_SIZE_8BIT, &v, 1, 100) == HAL_OK);
    CHECK(v == 0x68);
    CHECK(fake_app_wait(fa, VMCU_MSG_I2C_XFER, -1, -1, 1, 1000, &f));
    CHECK(f.data[1] == 0x50 && vmcu_wire_get_u16(f.data + 2) == 1 && vmcu_wire_get_u16(f.data + 4) == 1);
    CHECK(f.data[8] == 0x75);

    /* Écriture puis relecture de deux registres. */
    uint8_t w[2] = { 0xA5, 0x5A };
    uint8_t r[2] = { 0 };
    CHECK(HAL_I2C_Mem_Write(&hi2c1, DEV, 0x10, I2C_MEMADD_SIZE_8BIT, w, 2, 100) == HAL_OK);
    CHECK(HAL_I2C_Mem_Read(&hi2c1, DEV, 0x10, I2C_MEMADD_SIZE_8BIT, r, 2, 100) == HAL_OK);
    CHECK(r[0] == 0xA5 && r[1] == 0x5A);

    /* Adresse mémoire sur 16 bits : poids fort d'abord (0x0012 -> 0x00 puis 0x12). */
    CHECK(HAL_I2C_Mem_Write(&hi2c1, DEV, 0x0012, I2C_MEMADD_SIZE_16BIT, w, 1, 100) == HAL_OK);
    CHECK(fake_app_wait(fa, VMCU_MSG_I2C_XFER, -1, -1, 1, 1000, &f));
    CHECK(vmcu_wire_get_u16(f.data + 2) == 3 && f.data[8] == 0x00 && f.data[9] == 0x12 && f.data[10] == 0xA5);

    /* Transfert maître simple, et NACK d'une adresse absente. */
    uint8_t cmd[2] = { 0x20, 0x01 };
    CHECK(HAL_I2C_Master_Transmit(&hi2c1, DEV, cmd, 2, 100) == HAL_OK);
    CHECK(HAL_I2C_Master_Transmit(&hi2c1, 0x3Cu << 1, cmd, 2, 100) == HAL_ERROR);
    CHECK(HAL_I2C_GetError(&hi2c1) == HAL_I2C_ERROR_AF);

    /* _IT : transaction immédiate, callback de fin depuis le fil IRQ. */
    v = 0;
    CHECK(HAL_I2C_Mem_Read_IT(&hi2c1, DEV, 0x75, I2C_MEMADD_SIZE_8BIT, &v, 1) == HAL_OK);
    CHECK(wait_for(&mem_rx_done, 1));
    CHECK(v == 0x68 && HAL_I2C_GetState(&hi2c1) == HAL_I2C_STATE_READY);

    /* _DMA : même chemin, callback MasterTxCplt. */
    CHECK(HAL_I2C_Master_Transmit_DMA(&hi2c1, DEV, cmd, 2) == HAL_OK);
    CHECK(wait_for(&master_tx_done, 1));

    /* _IT vers une adresse absente : ErrorCallback, ErrorCode AF. */
    CHECK(HAL_I2C_Master_Transmit_IT(&hi2c1, 0x3Cu << 1, cmd, 2) == HAL_OK);
    CHECK(wait_for(&errors, 1));
    CHECK(HAL_I2C_GetError(&hi2c1) == HAL_I2C_ERROR_AF);
    CHECK(atomic_load(&master_tx_done) == 1);

    /* Transfert en cours : HAL_BUSY (verrou d'interruption tenu : le callback attend). */
    vmcu_irq_lock();
    CHECK(HAL_I2C_Mem_Read_IT(&hi2c1, DEV, 0x75, I2C_MEMADD_SIZE_8BIT, &v, 1) == HAL_OK);
    CHECK(HAL_I2C_Mem_Read(&hi2c1, DEV, 0x75, I2C_MEMADD_SIZE_8BIT, &v, 1, 10) == HAL_BUSY);
    vmcu_irq_unlock();
    CHECK(wait_for(&mem_rx_done, 2));

    vmcu_shutdown();
    fake_app_stop(fa);
    printf("test_i2c : OK\n");
    return 0;
}
