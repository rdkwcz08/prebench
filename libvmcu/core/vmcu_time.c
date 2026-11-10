/*
 * libvmcu/core/vmcu_time.c — Temps de simulation et garde-fous CPU (D12, section 5.5).
 *
 * Le temps de simulation est l'horloge monotone du système, moins les périodes
 * de pause : vmcu_millis() ne saute pas après un RESUME. Le mode temps virtuel
 * (E6) remplacera vmcu__sim_ns sans toucher aux shims.
 */
#include "vmcu_internal.h"

#include <stdatomic.h>

static vmcu_plat_mutex_t *time_mx;
static uint64_t start_ns;
static uint64_t paused_total_ns;
static uint64_t paused_since_ns;
static bool     paused;

/* Garde-fous, par fil. */
static _Thread_local uint32_t t_calls;
static _Thread_local uint32_t t_polls;

void vmcu__time_init(void)
{
    time_mx = vmcu_plat_mutex_create(false);
    start_ns = vmcu_plat_monotonic_ns();
}

uint64_t vmcu__sim_ns(void)
{
    uint64_t now = vmcu_plat_monotonic_ns();
    if (time_mx == NULL) {
        return 0;
    }
    vmcu_plat_mutex_lock(time_mx);
    uint64_t ref = paused ? paused_since_ns : now;
    uint64_t v = ref - start_ns - paused_total_ns;
    vmcu_plat_mutex_unlock(time_mx);
    return v;
}

void vmcu__time_set_paused(bool on)
{
    uint64_t now = vmcu_plat_monotonic_ns();
    vmcu_plat_mutex_lock(time_mx);
    if (on && !paused) {
        paused = true;
        paused_since_ns = now;
    } else if (!on && paused) {
        paused_total_ns += now - paused_since_ns;
        paused = false;
    }
    vmcu_plat_mutex_unlock(time_mx);
}

/* --- Garde-fous ------------------------------------------------------------------ */

static void micro_pause(void)
{
    vmcu__flush();
    vmcu_plat_sleep_ns(VMCU_GUARD_PAUSE_NS);
    t_calls = 0;
    t_polls = 0;
}

void vmcu__guard_call(void)
{
    if (++t_calls >= VMCU_GUARD_CALLS) {
        vmcu__warn_once(VMCU_WARN_BUSY_LOOP, "busy-loop",
                        "boucle sans délai détectée : micro-pauses insérées pour protéger le CPU");
        micro_pause();
    }
}

void vmcu__guard_poll(bool changed)
{
    if (changed) {
        t_polls = 0;
        return;
    }
    if (++t_polls >= VMCU_GUARD_POLLS) {
        micro_pause();
    }
}

void vmcu__guard_reset(void)
{
    t_calls = 0;
    t_polls = 0;
}

/* --- API publique ------------------------------------------------------------------ */

uint32_t vmcu_millis(void)
{
    (void)vmcu__ensure_init();
    vmcu__checkpoint();
    vmcu__guard_call();
    return (uint32_t)(vmcu__sim_ns() / 1000000ull);
}

uint64_t vmcu_micros(void)
{
    (void)vmcu__ensure_init();
    vmcu__checkpoint();
    vmcu__guard_call();
    return vmcu__sim_ns() / 1000ull;
}

static void delay_ns(uint64_t ns)
{
    (void)vmcu__ensure_init();
    vmcu__checkpoint();
    vmcu__flush(); /* section 5.1 du protocole : le lot part avant le sommeil */
    vmcu__guard_reset();
    uint64_t target = vmcu__sim_ns() + ns;
    for (;;) {
        uint64_t now = vmcu__sim_ns();
        if (now >= target) {
            break;
        }
        uint64_t remaining = target - now;
        /* Par tranches de 10 ms au plus : une pause gèle le délai restant. */
        vmcu_plat_sleep_ns(remaining < 10000000ull ? remaining : 10000000ull);
        vmcu__checkpoint();
    }
}

void vmcu_delay_ms(uint32_t ms)
{
    delay_ns((uint64_t)ms * 1000000ull);
}

void vmcu_delay_us(uint32_t us)
{
    delay_ns((uint64_t)us * 1000ull);
}
