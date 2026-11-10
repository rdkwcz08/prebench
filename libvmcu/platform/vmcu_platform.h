/*
 * libvmcu/platform/vmcu_platform.h — Couche système de libvmcu (règle 16.2.6).
 *
 * Seuls vmcu_platform_posix.c (macOS, Linux) et vmcu_platform_win32.c
 * appellent des API du système. Le reste de libvmcu passe par ces fonctions.
 *
 * Différences notables :
 * - Canal : socket Unix (POSIX) / named pipe en E/S asynchrones (Windows,
 *   pour qu'une lecture bloquée n'empêche pas les écritures d'un autre fil).
 * - Sommeil : nanosleep (POSIX) / timer haute résolution (Windows, section 5.5).
 */
#ifndef VMCU_PLATFORM_H
#define VMCU_PLATFORM_H

#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

#if defined(__GNUC__) || defined(__clang__)
#define VMCU_NORETURN __attribute__((noreturn))
#else
#define VMCU_NORETURN
#endif

/* --- Résultats ------------------------------------------------------------ */

typedef enum {
    VMCU_PLAT_OK    =  0,
    VMCU_PLAT_RETRY = -1, /* serveur pas encore prêt : réessayer plus tard */
    VMCU_PLAT_ERROR = -2
} vmcu_plat_result_t;

/* --- Canal messages (client et, pour les tests, serveur) ------------------ */

typedef struct vmcu_plat_conn vmcu_plat_conn_t;
typedef struct vmcu_plat_listener vmcu_plat_listener_t;

/** Connexion à l'adresse transmise par PREBENCH_SOCKET. */
vmcu_plat_result_t vmcu_plat_connect(const char *address, vmcu_plat_conn_t **out);

/** Écrit tout le tampon. 0 = succès, -1 = canal fermé ou erreur. */
int vmcu_plat_write_all(vmcu_plat_conn_t *conn, const void *data, size_t len);

/** Lit au plus len octets. > 0 : octets lus ; 0 : canal fermé ; < 0 : erreur ou arrêt. */
long vmcu_plat_read(vmcu_plat_conn_t *conn, void *buf, size_t len);

/** Débloque une lecture en cours et les suivantes (sans libérer). */
void vmcu_plat_conn_shutdown(vmcu_plat_conn_t *conn);

/** Libère la connexion. */
void vmcu_plat_conn_close(vmcu_plat_conn_t *conn);

/** Serveur (tests uniquement : en production, le serveur est le cœur Rust). */
vmcu_plat_result_t vmcu_plat_listen(const char *address, vmcu_plat_listener_t **out);
vmcu_plat_result_t vmcu_plat_accept(vmcu_plat_listener_t *listener, vmcu_plat_conn_t **out);
void vmcu_plat_listener_close(vmcu_plat_listener_t *listener);

/** Adresse locale unique pour les tests (chemin de socket ou nom de pipe). */
void vmcu_plat_test_address(char *out, size_t cap);

/* --- Temps ------------------------------------------------------------------ */

/** Horloge monotone, en nanosecondes. */
uint64_t vmcu_plat_monotonic_ns(void);

/** Sommeil réel (jamais d'attente active). */
void vmcu_plat_sleep_ns(uint64_t ns);

/* --- Fils, verrous, conditions ------------------------------------------------ */

typedef struct vmcu_plat_thread vmcu_plat_thread_t;
typedef struct vmcu_plat_mutex vmcu_plat_mutex_t;
typedef struct vmcu_plat_cond vmcu_plat_cond_t;

typedef void (*vmcu_plat_thread_fn)(void *arg);

vmcu_plat_thread_t *vmcu_plat_thread_start(vmcu_plat_thread_fn fn, void *arg);
void vmcu_plat_thread_join(vmcu_plat_thread_t *thread);
bool vmcu_plat_thread_is_current(const vmcu_plat_thread_t *thread);

vmcu_plat_mutex_t *vmcu_plat_mutex_create(bool recursive);
void vmcu_plat_mutex_lock(vmcu_plat_mutex_t *mutex);
void vmcu_plat_mutex_unlock(vmcu_plat_mutex_t *mutex);

vmcu_plat_cond_t *vmcu_plat_cond_create(void);
void vmcu_plat_cond_wait(vmcu_plat_cond_t *cond, vmcu_plat_mutex_t *mutex);
/** Attente bornée. Renvoie false à l'expiration du délai. */
bool vmcu_plat_cond_timedwait(vmcu_plat_cond_t *cond, vmcu_plat_mutex_t *mutex, uint64_t timeout_ns);
void vmcu_plat_cond_signal(vmcu_plat_cond_t *cond);
void vmcu_plat_cond_broadcast(vmcu_plat_cond_t *cond);

/* --- Processus ----------------------------------------------------------------- */

/**
 * Priorité effective du processus, lue de l'intérieur (section 11.2) :
 * macOS « QoS utility, nice 0 », Linux « nice 10 », Windows « classe
 * inférieure à la normale, EcoQoS actif ».
 */
void vmcu_plat_priority(char *out, size_t cap);

uint32_t vmcu_plat_pid(void);

/** Pose une variable d'environnement (tests). 0 = succès. */
int vmcu_plat_setenv(const char *name, const char *value);

#endif /* VMCU_PLATFORM_H */
