/*
 * tests/latency/main.c — Mesure de la latence d'un aller-retour synchrone (section 8.1).
 *
 * 1. firmware <-> cœur Rust            (VMCU_PING_CORE)
 * 2. firmware <-> cœur <-> modèle      (VMCU_PING_WORKER, composant « echo » dans QuickJS)
 *
 * La seconde mesure suit le chemin réel d'une transaction I2C ou SPI et
 * tranche la question ouverte de la section 15. Résultats sur la sortie
 * standard (onglet Journal de Prebench) puis fin du programme.
 */
#include <stdio.h>
#include <stdlib.h>

#include "vmcu.h"

#define WARMUP    50
#define SAMPLES 2000

static int compare_u32(const void *a, const void *b)
{
    uint32_t x = *(const uint32_t *)a;
    uint32_t y = *(const uint32_t *)b;
    return (x > y) - (x < y);
}

static void measure(vmcu_ping_target_t target, const char *name)
{
    static uint32_t rtt[SAMPLES];
    uint32_t us = 0;
    for (int i = 0; i < WARMUP; ++i) {
        vmcu_status_t st = vmcu_ping(target, &us);
        if (st != VMCU_OK) {
            printf("%s : %s\n", name, vmcu_strerror(st));
            fflush(stdout);
            return;
        }
    }
    uint64_t sum = 0;
    for (int i = 0; i < SAMPLES; ++i) {
        if (vmcu_ping(target, &rtt[i]) != VMCU_OK) {
            printf("%s : échec à l'échantillon %d\n", name, i);
            fflush(stdout);
            return;
        }
        sum += rtt[i];
    }
    qsort(rtt, SAMPLES, sizeof rtt[0], compare_u32);
    printf("%s : médiane %u µs, 99e centile %u µs, min %u µs, max %u µs, moyenne %.1f µs (%d mesures)\n",
           name, (unsigned)rtt[SAMPLES / 2], (unsigned)rtt[SAMPLES * 99 / 100], (unsigned)rtt[0],
           (unsigned)rtt[SAMPLES - 1], (double)sum / SAMPLES, SAMPLES);
    fflush(stdout);
}

int main(void)
{
    if (vmcu_init(NULL, NULL) != VMCU_OK) {
        return 1;
    }
    printf("Mesure de latence : %d allers-retours par chemin\n", SAMPLES);
    fflush(stdout);
    measure(VMCU_PING_CORE, "firmware <-> cœur");
    measure(VMCU_PING_WORKER, "firmware <-> cœur <-> modèle");
    vmcu_shutdown();
    return 0;
}
