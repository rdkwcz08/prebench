/*
 * libvmcu/core/vmcu_diag.c — Avertissements et journal (onglet ⚠, règle 16.2.10).
 */
#include "vmcu_internal.h"

#include <stdarg.h>
#include <stdio.h>
#include <string.h>

#define WARN_ONCE_SLOTS 256
#define WARN_KEY_MAX    64

static vmcu_plat_mutex_t *diag_mx;
static char   warned[WARN_ONCE_SLOTS][WARN_KEY_MAX];
static size_t warned_count;
static bool   warned_full;

void vmcu__diag_init(void)
{
    diag_mx = vmcu_plat_mutex_create(false);
}

void vmcu__warn(uint16_t code, const char *text)
{
    fprintf(stderr, "⚠ %s\n", text);
    uint8_t buf[VMCU_WIRE_MAX_STR + 4];
    vmcu_wbuf_t w;
    vmcu_wbuf_init(&w, buf, sizeof buf);
    vmcu_wbuf_u16(&w, code);
    vmcu_wbuf_str(&w, text);
    (void)vmcu__send(VMCU_MSG_WARN, buf, w.len);
}

/* true si `key` n'avait pas encore été signalée (et la mémorise). */
static bool first_time(const char *key)
{
    bool first = true;
    vmcu_plat_mutex_lock(diag_mx);
    for (size_t i = 0; i < warned_count; ++i) {
        if (strncmp(warned[i], key, WARN_KEY_MAX - 1) == 0) {
            first = false;
            break;
        }
    }
    if (first) {
        if (warned_count < WARN_ONCE_SLOTS) {
            strncpy(warned[warned_count], key, WARN_KEY_MAX - 1);
            warned[warned_count][WARN_KEY_MAX - 1] = '\0';
            warned_count++;
        } else if (warned_full) {
            first = false; /* table pleine : on cesse d'avertir (tampon borné) */
        } else {
            warned_full = true;
        }
    }
    vmcu_plat_mutex_unlock(diag_mx);
    return first;
}

void vmcu__warn_once(uint16_t code, const char *key, const char *text)
{
    if (diag_mx != NULL && first_time(key)) {
        vmcu__warn(code, text);
    }
}

void vmcu_warn_unsimulated(const char *function_name)
{
    if (function_name == NULL || vmcu__ensure_init() != VMCU_OK || !first_time(function_name)) {
        return;
    }
    char text[WARN_KEY_MAX + 32];
    (void)snprintf(text, sizeof text, "%s non simulé", function_name);
    fprintf(stderr, "⚠ %s\n", text);
    uint8_t buf[VMCU_WIRE_MAX_STR + 4];
    vmcu_wbuf_t w;
    vmcu_wbuf_init(&w, buf, sizeof buf);
    vmcu_wbuf_u16(&w, VMCU_WARN_UNSIMULATED);
    vmcu_wbuf_str(&w, function_name); /* l'app compose le message à partir du nom */
    (void)vmcu__send(VMCU_MSG_WARN, buf, w.len);
}

void vmcu_log(vmcu_log_level_t level, const char *fmt, ...)
{
    if (fmt == NULL || vmcu__ensure_init() != VMCU_OK) {
        return;
    }
    char text[VMCU_WIRE_MAX_STR + 1];
    va_list ap;
    va_start(ap, fmt);
    (void)vsnprintf(text, sizeof text, fmt, ap);
    va_end(ap);
    uint8_t buf[VMCU_WIRE_MAX_STR + 4];
    vmcu_wbuf_t w;
    vmcu_wbuf_init(&w, buf, sizeof buf);
    vmcu_wbuf_u8(&w, (uint8_t)level);
    vmcu_wbuf_u8(&w, 0);
    vmcu_wbuf_str(&w, text);
    (void)vmcu__send(VMCU_MSG_LOG, buf, w.len);
}
