/*
 * libvmcu/tests/test_spi.c — SPI du HAL STM32 et flux bulk de libvmcu, face à
 * une fausse app dont l'esclave répond le complément de chaque octet émis
 * (phase 3, étape 6) :
 * SPI_CONFIG (mode, fréquence), TransmitReceive et Receive synchrones,
 * Transmit sans attente, trames de 16 bits (poids fort d'abord), _IT et _DMA
 * avec leurs callbacks (demi-transfert compris pour _DMA), réception de plus
 * de 4096 octets découpée, écritures de plus de 4096 octets par la mémoire
 * partagée (flux bulk saturé puis vidé par un PING), ordre des octets reçus.
 */
#include <stdatomic.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include "fake_app.h"
#include "stm32f4xx_hal.h"
#include "vmcu_platform.h"
#include "vmcu_wire.h"

static SPI_HandleTypeDef hspi1;
static atomic_int txrx_done;
static atomic_int tx_half;
static atomic_int tx_done;

void HAL_SPI_TxRxCpltCallback(SPI_HandleTypeDef *hspi)
{
    if (hspi == &hspi1) {
        atomic_fetch_add(&txrx_done, 1);
    }
}

void HAL_SPI_TxHalfCpltCallback(SPI_HandleTypeDef *hspi)
{
    if (hspi == &hspi1) {
        atomic_fetch_add(&tx_half, 1);
    }
}

void HAL_SPI_TxCpltCallback(SPI_HandleTypeDef *hspi)
{
    if (hspi == &hspi1) {
        atomic_fetch_add(&tx_done, 1);
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

/* Empreinte attendue, mise à jour comme la fausse app (FNV-1a). */
static uint32_t hash = 2166136261u;
static uint64_t sent;

static void expect_sent(const uint8_t *data, size_t len)
{
    for (size_t i = 0; i < len; ++i) {
        hash = (hash ^ data[i]) * 16777619u;
    }
    sent += len;
}

int main(void)
{
    fake_app_t *fa = fake_app_create(1);
    fake_app_run(fa);
    CHECK(HAL_Init() == HAL_OK);

    /* SPI_CONFIG : bus 1, mode 3 (CPOL 1, CPHA 1), PCLK2 / 16. */
    hspi1.Instance = SPI1;
    hspi1.Init.Mode = SPI_MODE_MASTER;
    hspi1.Init.Direction = SPI_DIRECTION_2LINES;
    hspi1.Init.DataSize = SPI_DATASIZE_8BIT;
    hspi1.Init.CLKPolarity = SPI_POLARITY_HIGH;
    hspi1.Init.CLKPhase = SPI_PHASE_2EDGE;
    hspi1.Init.NSS = SPI_NSS_SOFT;
    hspi1.Init.BaudRatePrescaler = SPI_BAUDRATEPRESCALER_16;
    hspi1.Init.FirstBit = SPI_FIRSTBIT_MSB;
    CHECK(HAL_SPI_Init(&hspi1) == HAL_OK);
    fake_frame_t f;
    CHECK(fake_app_wait(fa, VMCU_MSG_SPI_CONFIG, -1, -1, 1, 1000, &f));
    CHECK(f.data[0] == 1 && f.data[1] == 3 && f.data[2] == 0);
    CHECK(vmcu_wire_get_u32(f.data + 4) == HAL_RCC_GetPCLK2Freq() / 16u);

    /* TransmitReceive : réponse synchrone du modèle ; CS non précisé (0xFF). */
    uint8_t tx[4] = { 0x9F, 0x00, 0x12, 0xFF };
    uint8_t rx[4] = { 0 };
    CHECK(HAL_SPI_TransmitReceive(&hspi1, tx, rx, 4, 100) == HAL_OK);
    CHECK(rx[0] == 0x60 && rx[1] == 0xFF && rx[2] == 0xED && rx[3] == 0x00);
    expect_sent(tx, 4);
    CHECK(fake_app_wait(fa, VMCU_MSG_SPI_XFER, -1, -1, 1, 1000, &f));
    CHECK(f.data[0] == 1 && f.data[1] == 0xFF && f.data[2] == 0xFF && f.data[3] == 0x01);

    /* Receive : 0xFF émis (NO_TX), l'esclave répond 0x00. */
    memset(rx, 0xAA, sizeof rx);
    CHECK(HAL_SPI_Receive(&hspi1, rx, 3, 100) == HAL_OK);
    CHECK(rx[0] == 0x00 && rx[1] == 0x00 && rx[2] == 0x00 && rx[3] == 0xAA);

    /* Transmit : écriture seule, sans attente. */
    uint8_t cmd[3] = { 0x2A, 0x00, 0xEF };
    CHECK(HAL_SPI_Transmit(&hspi1, cmd, 3, 100) == HAL_OK);
    expect_sent(cmd, 3);

    /* 16 bits : mot 0x1234 émis 0x12 puis 0x34 ; réponse remise en mots. */
    hspi1.Init.DataSize = SPI_DATASIZE_16BIT;
    uint16_t wtx[2] = { 0x1234, 0xABCD };
    uint16_t wrx[2] = { 0 };
    CHECK(HAL_SPI_TransmitReceive(&hspi1, (uint8_t *)wtx, (uint8_t *)wrx, 2, 100) == HAL_OK);
    CHECK(wrx[0] == (uint16_t)~0x1234u && wrx[1] == (uint16_t)~0xABCDu);
    const uint8_t wire16[4] = { 0x12, 0x34, 0xAB, 0xCD };
    expect_sent(wire16, 4);
    hspi1.Init.DataSize = SPI_DATASIZE_8BIT;

    /* _IT : transaction immédiate, TxRxCplt depuis le fil IRQ. */
    CHECK(HAL_SPI_TransmitReceive_IT(&hspi1, tx, rx, 4) == HAL_OK);
    CHECK(wait_for(&txrx_done, 1));
    CHECK(rx[0] == 0x60 && HAL_SPI_GetState(&hspi1) == HAL_SPI_STATE_READY);
    expect_sent(tx, 4);

    /* _DMA : demi-transfert puis fin. */
    CHECK(HAL_SPI_Transmit_DMA(&hspi1, cmd, 3) == HAL_OK);
    CHECK(wait_for(&tx_done, 1));
    CHECK(atomic_load(&tx_half) == 1);
    expect_sent(cmd, 3);

    /* Transfert en cours : HAL_BUSY (verrou d'interruption tenu : le callback attend). */
    vmcu_irq_lock();
    CHECK(HAL_SPI_Transmit_IT(&hspi1, cmd, 3) == HAL_OK);
    CHECK(HAL_SPI_Transmit(&hspi1, cmd, 3, 10) == HAL_BUSY);
    vmcu_irq_unlock();
    CHECK(wait_for(&tx_done, 2));
    expect_sent(cmd, 3);

    /* Réception de 5000 octets : deux transactions synchrones (4096 + 904). */
    static uint8_t big_tx[5000];
    static uint8_t big_rx[5000];
    for (size_t i = 0; i < sizeof big_tx; ++i) {
        big_tx[i] = (uint8_t)(i * 7u);
    }
    /* Point de synchronisation : les écritures asynchrones précédentes, encore
     * dans le lot, sont envoyées et traitées avant le compte de référence. */
    CHECK(vmcu_ping(VMCU_PING_CORE, NULL) == VMCU_OK);
    unsigned before = fake_app_count(fa, VMCU_MSG_SPI_XFER, -1, -1);
    CHECK(vmcu_spi_xfer(1, VMCU_PIN_NONE, VMCU_PIN_NONE, big_tx, big_rx, sizeof big_tx, 0) == VMCU_OK);
    CHECK(fake_app_count(fa, VMCU_MSG_SPI_XFER, -1, -1) == before + 2);
    for (size_t i = 0; i < sizeof big_rx; ++i) {
        CHECK(big_rx[i] == (uint8_t)~big_tx[i]);
    }
    expect_sent(big_tx, sizeof big_tx);

    /* Écritures de 150 000 octets (trame TFT 320 × 240 en RGB565, environ) :
     * flux bulk de 256 Kio ; la deuxième ne tient pas tant que le cœur n'a pas
     * lu la première : PING, puis écriture. Ordre des octets conservé. */
    static uint8_t frame[150000];
    for (int k = 0; k < 3; ++k) {
        for (size_t i = 0; i < sizeof frame; ++i) {
            frame[i] = (uint8_t)(i + (size_t)k * 31u);
        }
        CHECK(vmcu_spi_xfer(1, VMCU_PIN_NONE, VMCU_PIN_NONE, frame, NULL, sizeof frame, 0) == VMCU_OK);
        expect_sent(frame, sizeof frame);
    }
    CHECK(fake_app_wait(fa, VMCU_MSG_STREAM_OPEN, -1, -1, 1, 1000, &f));
    CHECK(vmcu_ping(VMCU_PING_CORE, NULL) == VMCU_OK); /* tout est traité */

    uint64_t bytes = 0;
    uint32_t got = 0;
    unsigned bulk = 0;
    fake_app_spi_stats(fa, &bytes, &got, &bulk);
    CHECK(bulk == 3);
    CHECK(bytes == sent);
    CHECK(got == hash);

    /* Au-delà de 16 Mio : refusé sans rien envoyer. */
    CHECK(vmcu_spi_xfer(1, VMCU_PIN_NONE, VMCU_PIN_NONE, frame, NULL, (size_t)17 * 1024 * 1024, 0)
          == VMCU_ERR_INVALID_ARG);

    vmcu_shutdown();
    fake_app_stop(fa);
    printf("test_spi : OK\n");
    return 0;
}
