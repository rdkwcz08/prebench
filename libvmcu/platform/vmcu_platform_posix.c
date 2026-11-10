/*
 * libvmcu/platform/vmcu_platform_posix.c — Couche système macOS et Linux.
 *
 * Canal : socket Unix (AF_UNIX, SOCK_STREAM). SIGPIPE est neutralisé
 * (SO_NOSIGPIPE sur macOS, MSG_NOSIGNAL sur Linux) : une écriture vers une
 * app fermée renvoie une erreur au lieu de tuer le processus.
 */
#if defined(__APPLE__)
#define _DARWIN_C_SOURCE 1
#else
#define _GNU_SOURCE 1
#endif

#include "vmcu_platform.h"

#include <errno.h>
#if defined(__APPLE__)
#include <pthread/qos.h>
#endif
#include <pthread.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/resource.h>
#include <sys/socket.h>
#include <sys/un.h>
#include <time.h>
#include <unistd.h>

#if defined(__APPLE__)
#define VMCU_SEND_FLAGS 0
#else
#define VMCU_SEND_FLAGS MSG_NOSIGNAL
#endif

/* --- Canal ------------------------------------------------------------------- */

struct vmcu_plat_conn {
    int fd;
};

struct vmcu_plat_listener {
    int  fd;
    char path[sizeof(((struct sockaddr_un *)0)->sun_path)];
};

static void no_sigpipe(int fd)
{
#if defined(__APPLE__)
    int one = 1;
    (void)setsockopt(fd, SOL_SOCKET, SO_NOSIGPIPE, &one, sizeof one);
#else
    (void)fd;
#endif
}

static int fill_address(struct sockaddr_un *sa, const char *address)
{
    memset(sa, 0, sizeof *sa);
    sa->sun_family = AF_UNIX;
    size_t n = strlen(address);
    if (n == 0 || n >= sizeof sa->sun_path) {
        return -1; /* macOS : 104 octets au plus (docs/protocol.md, 1.1) */
    }
    memcpy(sa->sun_path, address, n + 1);
    return 0;
}

static vmcu_plat_conn_t *conn_new(int fd)
{
    vmcu_plat_conn_t *c = malloc(sizeof *c);
    if (c == NULL) {
        close(fd);
        return NULL;
    }
    c->fd = fd;
    return c;
}

vmcu_plat_result_t vmcu_plat_connect(const char *address, vmcu_plat_conn_t **out)
{
    struct sockaddr_un sa;
    if (fill_address(&sa, address) != 0) {
        return VMCU_PLAT_ERROR;
    }
    int fd = socket(AF_UNIX, SOCK_STREAM, 0);
    if (fd < 0) {
        return VMCU_PLAT_ERROR;
    }
    no_sigpipe(fd);
    if (connect(fd, (struct sockaddr *)&sa, sizeof sa) != 0) {
        int err = errno;
        close(fd);
        return (err == ENOENT || err == ECONNREFUSED) ? VMCU_PLAT_RETRY : VMCU_PLAT_ERROR;
    }
    *out = conn_new(fd);
    return (*out != NULL) ? VMCU_PLAT_OK : VMCU_PLAT_ERROR;
}

int vmcu_plat_write_all(vmcu_plat_conn_t *conn, const void *data, size_t len)
{
    const uint8_t *p = data;
    while (len > 0) {
        ssize_t n = send(conn->fd, p, len, VMCU_SEND_FLAGS);
        if (n < 0) {
            if (errno == EINTR) {
                continue;
            }
            return -1;
        }
        p += n;
        len -= (size_t)n;
    }
    return 0;
}

long vmcu_plat_read(vmcu_plat_conn_t *conn, void *buf, size_t len)
{
    for (;;) {
        ssize_t n = recv(conn->fd, buf, len, 0);
        if (n < 0 && errno == EINTR) {
            continue;
        }
        return (long)n;
    }
}

void vmcu_plat_conn_shutdown(vmcu_plat_conn_t *conn)
{
    (void)shutdown(conn->fd, SHUT_RDWR);
}

void vmcu_plat_conn_close(vmcu_plat_conn_t *conn)
{
    if (conn != NULL) {
        close(conn->fd);
        free(conn);
    }
}

vmcu_plat_result_t vmcu_plat_listen(const char *address, vmcu_plat_listener_t **out)
{
    struct sockaddr_un sa;
    if (fill_address(&sa, address) != 0) {
        return VMCU_PLAT_ERROR;
    }
    vmcu_plat_listener_t *l = malloc(sizeof *l);
    if (l == NULL) {
        return VMCU_PLAT_ERROR;
    }
    memcpy(l->path, sa.sun_path, sizeof l->path);
    (void)unlink(l->path);
    l->fd = socket(AF_UNIX, SOCK_STREAM, 0);
    if (l->fd < 0 || bind(l->fd, (struct sockaddr *)&sa, sizeof sa) != 0 || listen(l->fd, 1) != 0) {
        if (l->fd >= 0) {
            close(l->fd);
        }
        free(l);
        return VMCU_PLAT_ERROR;
    }
    *out = l;
    return VMCU_PLAT_OK;
}

vmcu_plat_result_t vmcu_plat_accept(vmcu_plat_listener_t *listener, vmcu_plat_conn_t **out)
{
    int fd;
    do {
        fd = accept(listener->fd, NULL, NULL);
    } while (fd < 0 && errno == EINTR);
    if (fd < 0) {
        return VMCU_PLAT_ERROR;
    }
    no_sigpipe(fd);
    *out = conn_new(fd);
    return (*out != NULL) ? VMCU_PLAT_OK : VMCU_PLAT_ERROR;
}

void vmcu_plat_listener_close(vmcu_plat_listener_t *listener)
{
    if (listener != NULL) {
        close(listener->fd);
        (void)unlink(listener->path);
        free(listener);
    }
}

void vmcu_plat_test_address(char *out, size_t cap)
{
    static unsigned counter = 0;
    (void)snprintf(out, cap, "/tmp/vmcu-test-%u-%u.sock", (unsigned)getpid(), counter++);
}

/* --- Temps ---------------------------------------------------------------------- */

uint64_t vmcu_plat_monotonic_ns(void)
{
    struct timespec ts;
    (void)clock_gettime(CLOCK_MONOTONIC, &ts);
    return (uint64_t)ts.tv_sec * 1000000000ull + (uint64_t)ts.tv_nsec;
}

void vmcu_plat_sleep_ns(uint64_t ns)
{
    struct timespec req = {
        .tv_sec = (time_t)(ns / 1000000000ull),
        .tv_nsec = (long)(ns % 1000000000ull),
    };
    struct timespec rem;
    while (nanosleep(&req, &rem) != 0 && errno == EINTR) {
        req = rem;
    }
}

/* --- Fils ------------------------------------------------------------------------- */

struct vmcu_plat_thread {
    pthread_t           id;
    vmcu_plat_thread_fn fn;
    void               *arg;
};

static void *thread_trampoline(void *p)
{
    vmcu_plat_thread_t *t = p;
    t->fn(t->arg);
    return NULL;
}

vmcu_plat_thread_t *vmcu_plat_thread_start(vmcu_plat_thread_fn fn, void *arg)
{
    vmcu_plat_thread_t *t = malloc(sizeof *t);
    if (t == NULL) {
        return NULL;
    }
    t->fn = fn;
    t->arg = arg;
    if (pthread_create(&t->id, NULL, thread_trampoline, t) != 0) {
        free(t);
        return NULL;
    }
    return t;
}

void vmcu_plat_thread_join(vmcu_plat_thread_t *thread)
{
    if (thread != NULL) {
        (void)pthread_join(thread->id, NULL);
        free(thread);
    }
}

bool vmcu_plat_thread_is_current(const vmcu_plat_thread_t *thread)
{
    return thread != NULL && pthread_equal(thread->id, pthread_self());
}

struct vmcu_plat_mutex {
    pthread_mutex_t m;
};

vmcu_plat_mutex_t *vmcu_plat_mutex_create(bool recursive)
{
    vmcu_plat_mutex_t *mx = malloc(sizeof *mx);
    if (mx == NULL) {
        return NULL;
    }
    pthread_mutexattr_t attr;
    (void)pthread_mutexattr_init(&attr);
    if (recursive) {
        (void)pthread_mutexattr_settype(&attr, PTHREAD_MUTEX_RECURSIVE);
    }
    (void)pthread_mutex_init(&mx->m, &attr);
    (void)pthread_mutexattr_destroy(&attr);
    return mx;
}

void vmcu_plat_mutex_lock(vmcu_plat_mutex_t *mutex)
{
    (void)pthread_mutex_lock(&mutex->m);
}

void vmcu_plat_mutex_unlock(vmcu_plat_mutex_t *mutex)
{
    (void)pthread_mutex_unlock(&mutex->m);
}

struct vmcu_plat_cond {
    pthread_cond_t c;
};

vmcu_plat_cond_t *vmcu_plat_cond_create(void)
{
    vmcu_plat_cond_t *cv = malloc(sizeof *cv);
    if (cv == NULL) {
        return NULL;
    }
#if defined(__APPLE__)
    (void)pthread_cond_init(&cv->c, NULL);
#else
    pthread_condattr_t attr;
    (void)pthread_condattr_init(&attr);
    (void)pthread_condattr_setclock(&attr, CLOCK_MONOTONIC);
    (void)pthread_cond_init(&cv->c, &attr);
    (void)pthread_condattr_destroy(&attr);
#endif
    return cv;
}

void vmcu_plat_cond_wait(vmcu_plat_cond_t *cond, vmcu_plat_mutex_t *mutex)
{
    (void)pthread_cond_wait(&cond->c, &mutex->m);
}

bool vmcu_plat_cond_timedwait(vmcu_plat_cond_t *cond, vmcu_plat_mutex_t *mutex, uint64_t timeout_ns)
{
#if defined(__APPLE__)
    /* macOS : pas d'horloge monotone pour les conditions, délai relatif. */
    struct timespec rel = {
        .tv_sec = (time_t)(timeout_ns / 1000000000ull),
        .tv_nsec = (long)(timeout_ns % 1000000000ull),
    };
    return pthread_cond_timedwait_relative_np(&cond->c, &mutex->m, &rel) != ETIMEDOUT;
#else
    struct timespec abs;
    (void)clock_gettime(CLOCK_MONOTONIC, &abs);
    uint64_t ns = (uint64_t)abs.tv_nsec + timeout_ns % 1000000000ull;
    abs.tv_sec += (time_t)(timeout_ns / 1000000000ull + ns / 1000000000ull);
    abs.tv_nsec = (long)(ns % 1000000000ull);
    return pthread_cond_timedwait(&cond->c, &mutex->m, &abs) != ETIMEDOUT;
#endif
}

void vmcu_plat_cond_signal(vmcu_plat_cond_t *cond)
{
    (void)pthread_cond_signal(&cond->c);
}

void vmcu_plat_cond_broadcast(vmcu_plat_cond_t *cond)
{
    (void)pthread_cond_broadcast(&cond->c);
}

/* --- Processus ------------------------------------------------------------------------ */

uint32_t vmcu_plat_pid(void)
{
    return (uint32_t)getpid();
}

int vmcu_plat_setenv(const char *name, const char *value)
{
    return setenv(name, value, 1);
}

void vmcu_plat_priority(char *out, size_t cap)
{
    errno = 0;
    int nice_value = getpriority(PRIO_PROCESS, 0);
#if defined(__APPLE__)
    const char *qos;
    switch (qos_class_self()) {
    case QOS_CLASS_USER_INTERACTIVE: qos = "user-interactive"; break;
    case QOS_CLASS_USER_INITIATED:   qos = "user-initiated"; break;
    case QOS_CLASS_DEFAULT:          qos = "default"; break;
    case QOS_CLASS_UTILITY:          qos = "utility (cœurs économes)"; break;
    case QOS_CLASS_BACKGROUND:       qos = "background"; break;
    default:                         qos = "non précisée"; break;
    }
    (void)snprintf(out, cap, "QoS %s, nice %d", qos, nice_value);
#else
    (void)snprintf(out, cap, "nice %d", nice_value);
#endif
}
