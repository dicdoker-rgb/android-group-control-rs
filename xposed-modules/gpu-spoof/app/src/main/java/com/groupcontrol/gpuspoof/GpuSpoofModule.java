package com.groupcontrol.gpuspoof;

import de.robv.android.xposed.IXposedHookLoadPackage;
import de.robv.android.xposed.XC_MethodHook;
import de.robv.android.xposed.XposedBridge;
import de.robv.android.xposed.XposedHelpers;
import de.robv.android.xposed.callbacks.XC_LoadPackage;
import android.opengl.GLES20;
import android.os.SystemProperties;

public class GpuSpoofModule implements IXposedHookLoadPackage {

    // GPU profiles matching device platforms
    private String getGpuVendor() {
        String platform = SystemProperties.get("ro.board.platform", "");
        if (platform.startsWith("mt")) return "ARM";
        return "Qualcomm";
    }

    private String getGpuRenderer() {
        String platform = SystemProperties.get("ro.board.platform", "");
        switch (platform) {
            case "kona": case "sm8250": return "Adreno (TM) 650";
            case "lahaina": return "Adreno (TM) 660";
            case "taro": return "Adreno (TM) 730";
            case "kalama": return "Adreno (TM) 740";
            case "pineapple": return "Adreno (TM) 750";
            case "sm7150": return "Adreno (TM) 618";
            case "mt6895": return "Mali-G610 MC6";
            case "mt6985": return "Mali-G715-Immortalis MC11";
            default: return "Adreno (TM) 650";
        }
    }

    private String getGlVersion() {
        String platform = SystemProperties.get("ro.board.platform", "");
        if (platform.startsWith("mt")) return "OpenGL ES 3.2 v1.g18p0-01eac0";
        switch (platform) {
            case "kona": case "sm8250": return "OpenGL ES 3.2 V@0502.0 (GIT@a]1b2c3d4)";
            case "lahaina": return "OpenGL ES 3.2 V@0595.0 (GIT@e5f6g7h8)";
            case "taro": return "OpenGL ES 3.2 V@0615.0 (GIT@i9j0k1l2)";
            case "kalama": return "OpenGL ES 3.2 V@0725.0 (GIT@m3n4o5p6)";
            case "pineapple": return "OpenGL ES 3.2 V@0805.0 (GIT@q7r8s9t0)";
            default: return "OpenGL ES 3.2 V@0502.0";
        }
    }

    @Override
    public void handleLoadPackage(XC_LoadPackage.LoadPackageParam lpparam) throws Throwable {
        // Hook GLES20.glGetString
        try {
            XposedHelpers.findAndHookMethod(GLES20.class, "glGetString", int.class, new XC_MethodHook() {
                @Override
                protected void afterHookedMethod(MethodHookParam param) throws Throwable {
                    int name = (int) param.args[0];
                    if (name == GLES20.GL_VENDOR) {
                        param.setResult(getGpuVendor());
                    } else if (name == GLES20.GL_RENDERER) {
                        param.setResult(getGpuRenderer());
                    } else if (name == GLES20.GL_VERSION) {
                        param.setResult(getGlVersion());
                    }
                }
            });
            XposedBridge.log("[GpuSpoof] GLES20.glGetString hooked");
        } catch (Throwable t) {
            XposedBridge.log("[GpuSpoof] Failed to hook GLES20: " + t.getMessage());
        }

        // Hook EGL14.eglQueryString for EGL-level queries
        try {
            Class<?> egl14 = XposedHelpers.findClass("android.opengl.EGL14", lpparam.classLoader);
            XposedHelpers.findAndHookMethod(egl14, "eglQueryString",
                XposedHelpers.findClass("android.opengl.EGLDisplay", lpparam.classLoader),
                int.class,
                new XC_MethodHook() {
                    @Override
                    protected void afterHookedMethod(MethodHookParam param) throws Throwable {
                        int name = (int) param.args[1];
                        String result = (String) param.getResult();
                        if (result != null && name == 0x3053) { // EGL_VENDOR
                            param.setResult(result.replace("ARM", getGpuVendor()).replace("Mali", getGpuRenderer()));
                        }
                    }
                });
            XposedBridge.log("[GpuSpoof] EGL14.eglQueryString hooked");
        } catch (Throwable t) {
            XposedBridge.log("[GpuSpoof] Failed to hook EGL14: " + t.getMessage());
        }

        // Hook SystemProperties.get for GPU-related props
        try {
            XposedHelpers.findAndHookMethod("android.os.SystemProperties", lpparam.classLoader,
                "get", String.class, String.class, new XC_MethodHook() {
                    @Override
                    protected void afterHookedMethod(MethodHookParam param) throws Throwable {
                        String key = (String) param.args[0];
                        if ("ro.hardware.egl".equals(key)) {
                            String platform = SystemProperties.get("ro.board.platform", "");
                            param.setResult(platform.startsWith("mt") ? "mali" : "adreno");
                        }
                    }
                });
        } catch (Throwable t) {}
    }
}
