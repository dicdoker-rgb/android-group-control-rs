package com.groupcontrol.inputhide;

import android.hardware.input.InputManager;
import android.view.InputDevice;

import java.util.ArrayList;
import java.util.List;
import java.util.Random;

import de.robv.android.xposed.IXposedHookLoadPackage;
import de.robv.android.xposed.XC_MethodHook;
import de.robv.android.xposed.XposedBridge;
import de.robv.android.xposed.XposedHelpers;
import de.robv.android.xposed.callbacks.XC_LoadPackage;

public class InputHideModule implements IXposedHookLoadPackage {

    private static final String TAG = "InputHide";
    private final Random random = new Random();

    // Keywords that identify virtual/emulated/board input devices
    private static final String[] VIRTUAL_KEYWORDS = {
            "virtual", "uinput", "rockchip", "rk805", "pwm", "dp0", "hdmi"
    };

    // Keywords that identify board-level devices to filter from ID list
    private static final String[] FILTER_KEYWORDS = {
            "rockchip", "hdmi", "dp0"
    };

    // Realistic touch controller names for replacement
    private static final String[] REAL_TOUCH_NAMES = {
            "synaptics_dsx",
            "sec_touchscreen",
            "goodix_ts",
            "fts_ts"
    };

    @Override
    public void handleLoadPackage(XC_LoadPackage.LoadPackageParam lpparam) throws Throwable {
        XposedBridge.log(TAG + ": Loaded in " + lpparam.packageName);

        hookGetInputDevice(lpparam);
        hookGetInputDeviceIds(lpparam);
    }

    /**
     * Hook 1: InputManager.getInputDevice(int id) — replace virtual device names
     *
     * If the returned InputDevice name contains any virtual/emulated keyword,
     * replace the name with a realistic touch controller name.
     */
    private void hookGetInputDevice(XC_LoadPackage.LoadPackageParam lpparam) {
        try {
            XposedHelpers.findAndHookMethod(
                    "android.hardware.input.InputManager",
                    lpparam.classLoader,
                    "getInputDevice",
                    int.class,
                    new XC_MethodHook() {
                        @Override
                        protected void afterHookedMethod(MethodHookParam param) throws Throwable {
                            try {
                                InputDevice device = (InputDevice) param.getResult();
                                if (device == null) return;

                                String deviceName = device.getName();
                                if (deviceName == null) return;

                                String lowerName = deviceName.toLowerCase();
                                boolean isVirtual = false;

                                for (String keyword : VIRTUAL_KEYWORDS) {
                                    if (lowerName.contains(keyword)) {
                                        isVirtual = true;
                                        break;
                                    }
                                }

                                if (isVirtual) {
                                    // Pick a deterministic replacement name based on device ID
                                    int deviceId = (int) param.args[0];
                                    String fakeName = REAL_TOUCH_NAMES[
                                            Math.abs(deviceId) % REAL_TOUCH_NAMES.length];

                                    // Replace the name field via reflection
                                    XposedHelpers.setObjectField(device, "mName", fakeName);

                                    XposedBridge.log(TAG + ": Renamed device '" + deviceName
                                            + "' -> '" + fakeName + "' (id=" + deviceId + ")");
                                }
                            } catch (Throwable t) {
                                XposedBridge.log(TAG + ": Error in getInputDevice hook: " + t.getMessage());
                            }
                        }
                    }
            );
            XposedBridge.log(TAG + ": Hooked InputManager.getInputDevice");
        } catch (Throwable t) {
            XposedBridge.log(TAG + ": Failed to hook getInputDevice: " + t.getMessage());
        }
    }

    /**
     * Hook 2: InputManager.getInputDeviceIds() — filter out board-level device IDs
     *
     * Removes device IDs whose names contain "rockchip", "hdmi", "dp0".
     * Only keeps IDs that look like real touch/button devices.
     */
    private void hookGetInputDeviceIds(XC_LoadPackage.LoadPackageParam lpparam) {
        try {
            XposedHelpers.findAndHookMethod(
                    "android.hardware.input.InputManager",
                    lpparam.classLoader,
                    "getInputDeviceIds",
                    new XC_MethodHook() {
                        @Override
                        protected void afterHookedMethod(MethodHookParam param) throws Throwable {
                            try {
                                int[] originalIds = (int[]) param.getResult();
                                if (originalIds == null || originalIds.length == 0) return;

                                InputManager inputManager = (InputManager) param.thisObject;
                                List<Integer> filteredIds = new ArrayList<>();

                                for (int id : originalIds) {
                                    boolean shouldFilter = false;

                                    try {
                                        // Get the device to check its name
                                        // Use the native method directly to avoid recursion
                                        InputDevice device = (InputDevice) XposedHelpers.callMethod(
                                                inputManager, "getInputDevice", id);

                                        if (device != null) {
                                            String name = device.getName();
                                            if (name != null) {
                                                String lowerName = name.toLowerCase();
                                                for (String keyword : FILTER_KEYWORDS) {
                                                    if (lowerName.contains(keyword)) {
                                                        shouldFilter = true;
                                                        XposedBridge.log(TAG + ": Filtering device '"
                                                                + name + "' (id=" + id + ")");
                                                        break;
                                                    }
                                                }
                                            }
                                        }
                                    } catch (Throwable t) {
                                        // If we can't read the device, keep it
                                        XposedBridge.log(TAG + ": Could not read device id=" + id
                                                + ", keeping it");
                                    }

                                    if (!shouldFilter) {
                                        filteredIds.add(id);
                                    }
                                }

                                // Convert back to int array
                                int[] result = new int[filteredIds.size()];
                                for (int i = 0; i < filteredIds.size(); i++) {
                                    result[i] = filteredIds.get(i);
                                }

                                param.setResult(result);

                            } catch (Throwable t) {
                                XposedBridge.log(TAG + ": Error in getInputDeviceIds hook: " + t.getMessage());
                            }
                        }
                    }
            );
            XposedBridge.log(TAG + ": Hooked InputManager.getInputDeviceIds");
        } catch (Throwable t) {
            XposedBridge.log(TAG + ": Failed to hook getInputDeviceIds: " + t.getMessage());
        }
    }
}
