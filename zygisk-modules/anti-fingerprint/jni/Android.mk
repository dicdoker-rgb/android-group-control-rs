# Android.mk for anti-fingerprint Zygisk module

LOCAL_PATH := $(call my-dir)

# Dobby 静态库
include $(CLEAR_VARS)
LOCAL_MODULE := dobby
LOCAL_SRC_FILES := dobby/libs/$(TARGET_ARCH_ABI)/libdobby.a
LOCAL_EXPORT_C_INCLUDES := $(LOCAL_PATH)/dobby/include
include $(PREBUILT_STATIC_LIBRARY)

# 主模块
include $(CLEAR_VARS)
LOCAL_MODULE := anti_fingerprint
LOCAL_SRC_FILES := main.cpp
LOCAL_STATIC_LIBRARIES := dobby
LOCAL_LDLIBS := -llog -ldl
LOCAL_CFLAGS := -O2 -fvisibility=hidden
LOCAL_CPPFLAGS := -std=c++17
include $(BUILD_SHARED_LIBRARY)
