/*
 * GL String Spoofer - Safe LD_PRELOAD library
 * Only hooks when GL library is actually loaded (lazy init)
 * Won't crash init/servicemanager which don't use GL
 */
#define _GNU_SOURCE
#include <dlfcn.h>
#include <string.h>
#include <stdlib.h>
#include <unistd.h>

#define GL_VENDOR     0x1F00
#define GL_RENDERER   0x1F01
#define GL_VERSION    0x1F02

#define EGL_VENDOR    0x3053
#define EGL_VERSION   0x3054

typedef const char* (*glGetString_t)(unsigned int name);
typedef const char* (*eglQueryString_t)(void* dpy, int name);

static glGetString_t orig_glGetString = NULL;
static eglQueryString_t orig_eglQueryString = NULL;
static int gl_init_done = 0;
static int egl_init_done = 0;

static void lazy_init_gl(void) {
    if (gl_init_done) return;
    gl_init_done = 1;
    /* Try to find the real function - if GL not loaded, returns NULL */
    orig_glGetString = (glGetString_t)dlsym(RTLD_NEXT, "glGetString");
}

static void lazy_init_egl(void) {
    if (egl_init_done) return;
    egl_init_done = 1;
    orig_eglQueryString = (eglQueryString_t)dlsym(RTLD_NEXT, "eglQueryString");
}

const char* glGetString(unsigned int name) {
    lazy_init_gl();

    switch (name) {
        case GL_VENDOR:
            return "Qualcomm";
        case GL_RENDERER:
            return "Adreno (TM) 650";
        case GL_VERSION:
            return "OpenGL ES 3.2 V@0502.0 (GIT@9deb42f, I3a5765dd96, 1609190895)";
        default:
            if (orig_glGetString)
                return orig_glGetString(name);
            return "";
    }
}

const char* eglQueryString(void* dpy, int name) {
    lazy_init_egl();

    switch (name) {
        case EGL_VENDOR:
            return "Qualcomm Inc.";
        case EGL_VERSION:
            return "1.5 Adreno";
        default:
            if (orig_eglQueryString)
                return orig_eglQueryString(dpy, name);
            return "";
    }
}
