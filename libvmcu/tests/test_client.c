/*
 * libvmcu/tests/test_client.c — libvmcu face à une fausse app : poignée de main,
 * état initial, fronts, lots, ordre, ping, délais, pause, avertissements.
 */
#include <stdatomic.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include "fake_app.h"
#include "vmcu.h"
#include "vmcu_platform.h"
#include "vmcu_wire.h"

static atomic_int edge_count;
static atomic_int last_edge;

static void on_edge(uint8_t port, uint8_t pin, vmcu_gpio_edge_t edge, void *user)
{
    (void)user;
    if (port == 2 && pin == 13) {
        atomic_store(&last_edge, (int)edge);
        atomic_fetch_add(&edge_count, 1);
    }
}

static bool wait_edges(int expected, uint32_t timeout_ms)
{
    for (uint32_t t = 0; t < timeout_ms; t += 5) {
        if (atomic_load(&edge_count) >= expected) {
            return true;
        }
        vmcu_plat_sleep_ns(5000000ull);
    }
    return atomic_load(&edge_count) >= expected;
}

int main(void)
{
    fake_app_t *fa = fake_app_create(7);
    fake_app_initial_input(fa, 2, 13, 1); /* B1 relâché : niveau haut */
    fake_app_run(fa);

    /* Poignée de main */
    CHECK(vmcu_init("nucleo-f401re", "stm32-hal") == VMCU_OK);
    CHECK(vmcu_init(NULL, NULL) == VMCU_OK); /* idempotente */
    CHECK(vmcu_board_id() == 7);
    CHECK(vmcu_adc_bits() == 12);
    CHECK(strcmp(fake_app_hello_board(fa), "nucleo-f401re") == 0);
    CHECK(strcmp(fake_app_hello_framework(fa), "stm32-hal") == 0);

    /* État initial reçu avant WELCOME */
    CHECK(vmcu_gpio_read(2, 13) == VMCU_HIGH);

    /* Front descendant -> callback ; front montant ignoré */
    CHECK(vmcu_gpio_config(2, 13, VMCU_GPIO_MODE_INPUT, VMCU_GPIO_PULL_UP, VMCU_GPIO_AF_NONE) == VMCU_OK);
    CHECK(vmcu_gpio_on_edge(2, 13, VMCU_GPIO_EDGE_FALLING, on_edge, NULL) == VMCU_OK);
    fake_frame_t f;
    CHECK(fake_app_wait(fa, VMCU_MSG_GPIO_CONFIG, 2, 13, 2, 1000, &f));
    CHECK(f.data[2] == VMCU_GPIO_MODE_INPUT && f.data[3] == VMCU_GPIO_PULL_UP);
    CHECK(f.data[4] == VMCU_GPIO_EDGE_FALLING && f.data[5] == VMCU_GPIO_AF_NONE);
    fake_app_send_gpio_input(fa, 2, 13, 0);
    CHECK(wait_edges(1, 2000));
    CHECK(atomic_load(&last_edge) == VMCU_GPIO_EDGE_FALLING);
    CHECK(vmcu_gpio_read(2, 13) == VMCU_LOW);
    fake_app_send_gpio_input(fa, 2, 13, 1);
    vmcu_plat_sleep_ns(100000000ull);
    CHECK(atomic_load(&edge_count) == 1);
    CHECK(vmcu_gpio_read(2, 13) == VMCU_HIGH);

    /* Lot vidé en moins de 1 ms sans transaction synchrone (marge CI : 500 ms) */
    CHECK(vmcu_gpio_write(0, 5, VMCU_HIGH) == VMCU_OK);
    CHECK(fake_app_wait(fa, VMCU_MSG_GPIO_WRITE, 0, 5, 1, 500, &f));
    CHECK(f.data[2] == 1);
    CHECK(vmcu_gpio_read(0, 5) == VMCU_HIGH); /* sortie : dernier niveau écrit */

    /* Ordre : l'écriture précède le PING qui la suit */
    CHECK(vmcu_gpio_write(0, 5, VMCU_LOW) == VMCU_OK);
    uint32_t rtt = 0;
    CHECK(vmcu_ping(VMCU_PING_CORE, &rtt) == VMCU_OK);
    fake_frame_t w2, ping;
    CHECK(fake_app_wait(fa, VMCU_MSG_GPIO_WRITE, 0, 5, 2, 100, &w2));
    CHECK(fake_app_wait(fa, VMCU_MSG_PING, -1, -1, 1, 100, &ping));
    CHECK(w2.index < ping.index);
    CHECK((ping.flags & VMCU_FLAG_SYNC) != 0);
    CHECK(rtt < 1000000u);

    /* Ping Worker : sans composant d'écho, puis avec */
    CHECK(vmcu_ping(VMCU_PING_WORKER, &rtt) == VMCU_ERR_UNSUPPORTED);
    fake_app_set_echo(fa, true);
    CHECK(vmcu_ping(VMCU_PING_WORKER, &rtt) == VMCU_OK);

    /* Délai : vrai sommeil d'au moins 20 ms */
    uint64_t t0 = vmcu_micros();
    vmcu_delay_ms(20);
    uint64_t dt = vmcu_micros() - t0;
    CHECK(dt >= 20000u && dt < 500000u);

    /* Pause : le temps de simulation est gelé */
    uint32_t m0 = vmcu_millis();
    fake_app_send_empty(fa, VMCU_MSG_PAUSE);
    vmcu_plat_sleep_ns(300000000ull); /* 300 ms réelles en pause */
    fake_app_send_empty(fa, VMCU_MSG_RESUME);
    vmcu_plat_sleep_ns(20000000ull);
    uint32_t m1 = vmcu_millis();
    CHECK(m1 - m0 < 200u);

    /* Avertissement « non simulé » envoyé une seule fois */
    vmcu_warn_unsimulated("HAL_Exemple");
    vmcu_warn_unsimulated("HAL_Exemple");
    vmcu_plat_sleep_ns(50000000ull);
    CHECK(fake_app_count(fa, VMCU_MSG_WARN, -1, -1) == 1);

    vmcu_shutdown();
    CHECK(vmcu_gpio_write(0, 5, VMCU_HIGH) == VMCU_ERR_DISCONNECTED);
    fake_app_stop(fa);
    printf("test_client : OK\n");
    return 0;
}
