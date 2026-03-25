package com.groupcontrol.sensornoise;

import android.hardware.Sensor;
import android.hardware.SensorManager;
import android.os.SystemClock;

import java.util.ArrayList;
import java.util.List;
import java.util.Random;

import de.robv.android.xposed.IXposedHookLoadPackage;
import de.robv.android.xposed.XC_MethodHook;
import de.robv.android.xposed.XposedBridge;
import de.robv.android.xposed.XposedHelpers;
import de.robv.android.xposed.callbacks.XC_LoadPackage;

public class SensorNoiseModule implements IXposedHookLoadPackage {

    private static final String TAG = "SensorNoise";
    private final Random random = new Random();

    // Boot time reference for battery simulation
    private final long bootTimeMillis = System.currentTimeMillis();

    // Sensor type constants (matching android.hardware.Sensor)
    private static final int TYPE_ACCELEROMETER = 1;
    private static final int TYPE_MAGNETIC_FIELD = 2;
    private static final int TYPE_GYROSCOPE = 4;
    private static final int TYPE_LIGHT = 5;
    private static final int TYPE_PROXIMITY = 8;
    private static final int TYPE_GRAVITY = 9;
    private static final int TYPE_LINEAR_ACCELERATION = 10;

    @Override
    public void handleLoadPackage(XC_LoadPackage.LoadPackageParam lpparam) throws Throwable {
        XposedBridge.log(TAG + ": Loaded in " + lpparam.packageName);

        hookSensorEventDispatch(lpparam);
        hookBatteryIntent(lpparam);
        hookGetSensorList(lpparam);
    }

    /**
     * Hook 1: SensorEventQueue.dispatchSensorEvent() — inject noise into sensor readings
     */
    private void hookSensorEventDispatch(XC_LoadPackage.LoadPackageParam lpparam) {
        try {
            Class<?> sensorEventQueueClass = XposedHelpers.findClass(
                    "android.hardware.SystemSensorManager$SensorEventQueue",
                    lpparam.classLoader);

            XposedHelpers.findAndHookMethod(
                    sensorEventQueueClass,
                    "dispatchSensorEvent",
                    int.class, float[].class, int.class, long.class,
                    new XC_MethodHook() {
                        @Override
                        protected void beforeHookedMethod(MethodHookParam param) throws Throwable {
                            try {
                                int handle = (int) param.args[0];
                                float[] values = (float[]) param.args[1];

                                if (values == null || values.length == 0) return;

                                // Determine sensor type from handle
                                // The handle corresponds to the sensor's internal handle,
                                // we need to resolve the sensor type from it
                                int sensorType = resolveSensorType(param.thisObject, handle);

                                switch (sensorType) {
                                    case TYPE_ACCELEROMETER:
                                        // Add Gaussian noise with std=0.02 to X, Y, Z
                                        if (values.length >= 3) {
                                            values[0] += (float) (random.nextGaussian() * 0.02);
                                            values[1] += (float) (random.nextGaussian() * 0.02);
                                            values[2] += (float) (random.nextGaussian() * 0.02);
                                        }
                                        break;

                                    case TYPE_GYROSCOPE:
                                        // Add Gaussian noise with std=0.001 to X, Y, Z
                                        if (values.length >= 3) {
                                            values[0] += (float) (random.nextGaussian() * 0.001);
                                            values[1] += (float) (random.nextGaussian() * 0.001);
                                            values[2] += (float) (random.nextGaussian() * 0.001);
                                        }
                                        break;

                                    case TYPE_LIGHT:
                                        // Set to random value between 200-800 lux
                                        values[0] = 200.0f + random.nextFloat() * 600.0f;
                                        break;

                                    case TYPE_PROXIMITY:
                                        // Set to 5.0 (far)
                                        values[0] = 5.0f;
                                        break;

                                    default:
                                        // No modification for other sensor types
                                        break;
                                }

                                param.args[1] = values;

                            } catch (Throwable t) {
                                XposedBridge.log(TAG + ": Error in dispatchSensorEvent hook: " + t.getMessage());
                            }
                        }
                    }
            );
            XposedBridge.log(TAG + ": Hooked SensorEventQueue.dispatchSensorEvent");
        } catch (Throwable t) {
            XposedBridge.log(TAG + ": Failed to hook dispatchSensorEvent: " + t.getMessage());
        }
    }

    /**
     * Resolve sensor type from the SensorEventQueue handle.
     * The queue maintains a mapping of handles to sensors internally.
     */
    private int resolveSensorType(Object sensorEventQueue, int handle) {
        try {
            // Try to access the mSensorsEvents SparseArray which maps handle -> SensorEvent
            Object sparseArray = XposedHelpers.getObjectField(sensorEventQueue, "mSensorsEvents");
            if (sparseArray != null) {
                Object sensorEvent = XposedHelpers.callMethod(sparseArray, "get", handle);
                if (sensorEvent != null) {
                    Object sensor = XposedHelpers.getObjectField(sensorEvent, "sensor");
                    if (sensor != null) {
                        return (int) XposedHelpers.callMethod(sensor, "getType");
                    }
                }
            }
        } catch (Throwable t) {
            // Fallback: try alternative field names
            try {
                Object manager = XposedHelpers.getObjectField(sensorEventQueue, "mManager");
                if (manager != null) {
                    Object handleToSensor = XposedHelpers.getObjectField(manager, "mHandleToSensor");
                    if (handleToSensor != null) {
                        Object sensor = XposedHelpers.callMethod(handleToSensor, "get", handle);
                        if (sensor != null) {
                            return (int) XposedHelpers.callMethod(sensor, "getType");
                        }
                    }
                }
            } catch (Throwable t2) {
                XposedBridge.log(TAG + ": Could not resolve sensor type for handle " + handle);
            }
        }
        return -1; // Unknown
    }

    /**
     * Hook 2: Battery intent broadcast — inject dynamic temperature and level
     *
     * Hooks BatteryManager-related intent extras to simulate realistic battery behavior:
     * - Temperature increases with uptime: 250 (25C) + uptime_minutes * 0.3, capped at 380 (38C)
     * - Battery level slowly decreases over time
     */
    private void hookBatteryIntent(XC_LoadPackage.LoadPackageParam lpparam) {
        try {
            // Hook Intent.getIntExtra to intercept battery-related queries
            XposedHelpers.findAndHookMethod(
                    "android.content.Intent",
                    lpparam.classLoader,
                    "getIntExtra",
                    String.class, int.class,
                    new XC_MethodHook() {
                        @Override
                        protected void afterHookedMethod(MethodHookParam param) throws Throwable {
                            try {
                                Object intent = param.thisObject;
                                String action = (String) XposedHelpers.callMethod(intent, "getAction");

                                // Only intercept battery changed intents
                                if (!"android.intent.action.BATTERY_CHANGED".equals(action)) {
                                    return;
                                }

                                String name = (String) param.args[0];
                                long uptimeMinutes = SystemClock.elapsedRealtime() / 60000;

                                if ("temperature".equals(name)) {
                                    // Temperature: 250 (25C) + uptime_minutes * 0.3, cap at 380 (38C)
                                    int temperature = (int) Math.min(250 + uptimeMinutes * 0.3, 380);
                                    param.setResult(temperature);
                                } else if ("level".equals(name)) {
                                    // Battery level: start at 100, decrease ~1% every 10 minutes
                                    int level = (int) Math.max(100 - (uptimeMinutes / 10), 15);
                                    param.setResult(level);
                                } else if ("scale".equals(name)) {
                                    param.setResult(100);
                                } else if ("status".equals(name)) {
                                    // BATTERY_STATUS_DISCHARGING = 3
                                    param.setResult(3);
                                } else if ("plugged".equals(name)) {
                                    // Not plugged in
                                    param.setResult(0);
                                } else if ("health".equals(name)) {
                                    // BATTERY_HEALTH_GOOD = 2
                                    param.setResult(2);
                                }
                            } catch (Throwable t) {
                                XposedBridge.log(TAG + ": Error in battery intent hook: " + t.getMessage());
                            }
                        }
                    }
            );
            XposedBridge.log(TAG + ": Hooked battery intent extras");
        } catch (Throwable t) {
            XposedBridge.log(TAG + ": Failed to hook battery intent: " + t.getMessage());
        }
    }

    /**
     * Hook 3: SensorManager.getSensorList() — ensure a complete set of sensors is reported
     *
     * Reports at minimum: Accelerometer, Gyroscope, Magnetic Field, Light, Proximity,
     * Gravity, Linear Acceleration.
     */
    private void hookGetSensorList(XC_LoadPackage.LoadPackageParam lpparam) {
        try {
            XposedHelpers.findAndHookMethod(
                    "android.hardware.SensorManager",
                    lpparam.classLoader,
                    "getSensorList",
                    int.class,
                    new XC_MethodHook() {
                        @Override
                        protected void afterHookedMethod(MethodHookParam param) throws Throwable {
                            try {
                                int requestedType = (int) param.args[0];

                                // Only intercept TYPE_ALL requests
                                if (requestedType != Sensor.TYPE_ALL) {
                                    return;
                                }

                                @SuppressWarnings("unchecked")
                                List<Sensor> originalList = (List<Sensor>) param.getResult();
                                if (originalList == null) {
                                    originalList = new ArrayList<>();
                                }

                                // Required sensor types and their names
                                int[][] requiredSensors = {
                                        {TYPE_ACCELEROMETER, 1},      // Accelerometer
                                        {TYPE_MAGNETIC_FIELD, 2},     // Magnetic Field
                                        {TYPE_GYROSCOPE, 4},          // Gyroscope
                                        {TYPE_LIGHT, 5},              // Light
                                        {TYPE_PROXIMITY, 8},          // Proximity
                                        {TYPE_GRAVITY, 9},            // Gravity
                                        {TYPE_LINEAR_ACCELERATION, 10}, // Linear Acceleration
                                };

                                String[] sensorNames = {
                                        "BMI160 Accelerometer",
                                        "AKM09918 Magnetometer",
                                        "BMI160 Gyroscope",
                                        "TMD2725 Light Sensor",
                                        "TMD2725 Proximity Sensor",
                                        "Gravity Sensor",
                                        "Linear Acceleration Sensor",
                                };

                                String[] vendorNames = {
                                        "Bosch",
                                        "AKM",
                                        "Bosch",
                                        "AMS",
                                        "AMS",
                                        "QTI",
                                        "QTI",
                                };

                                // Check which sensor types are already present
                                List<Integer> presentTypes = new ArrayList<>();
                                for (Sensor s : originalList) {
                                    presentTypes.add(s.getType());
                                }

                                // Add missing sensors
                                List<Sensor> augmentedList = new ArrayList<>(originalList);
                                for (int i = 0; i < requiredSensors.length; i++) {
                                    int sensorType = requiredSensors[i][0];
                                    if (!presentTypes.contains(sensorType)) {
                                        Sensor fakeSensor = createFakeSensor(
                                                lpparam.classLoader,
                                                sensorNames[i],
                                                vendorNames[i],
                                                sensorType
                                        );
                                        if (fakeSensor != null) {
                                            augmentedList.add(fakeSensor);
                                        }
                                    }
                                }

                                param.setResult(augmentedList);

                            } catch (Throwable t) {
                                XposedBridge.log(TAG + ": Error in getSensorList hook: " + t.getMessage());
                            }
                        }
                    }
            );
            XposedBridge.log(TAG + ": Hooked SensorManager.getSensorList");
        } catch (Throwable t) {
            XposedBridge.log(TAG + ": Failed to hook getSensorList: " + t.getMessage());
        }
    }

    /**
     * Create a fake Sensor object via reflection.
     */
    private Sensor createFakeSensor(ClassLoader classLoader, String name, String vendor, int type) {
        try {
            Class<?> sensorClass = XposedHelpers.findClass("android.hardware.Sensor", classLoader);
            Sensor sensor = (Sensor) sensorClass.getDeclaredConstructor().newInstance();

            XposedHelpers.setObjectField(sensor, "mName", name);
            XposedHelpers.setObjectField(sensor, "mVendor", vendor);
            XposedHelpers.setIntField(sensor, "mType", type);
            XposedHelpers.setIntField(sensor, "mVersion", 1);
            XposedHelpers.setFloatField(sensor, "mMaxRange", getMaxRangeForType(type));
            XposedHelpers.setFloatField(sensor, "mResolution", getResolutionForType(type));
            XposedHelpers.setFloatField(sensor, "mPower", 0.5f);
            XposedHelpers.setIntField(sensor, "mMinDelay", 5000); // 5ms

            return sensor;
        } catch (Throwable t) {
            XposedBridge.log(TAG + ": Failed to create fake sensor type " + type + ": " + t.getMessage());
            return null;
        }
    }

    private float getMaxRangeForType(int type) {
        switch (type) {
            case TYPE_ACCELEROMETER: return 39.2266f;   // ~4g
            case TYPE_MAGNETIC_FIELD: return 4912.0f;
            case TYPE_GYROSCOPE: return 34.9066f;       // ~2000 deg/s
            case TYPE_LIGHT: return 60000.0f;
            case TYPE_PROXIMITY: return 5.0f;
            case TYPE_GRAVITY: return 9.80665f;
            case TYPE_LINEAR_ACCELERATION: return 39.2266f;
            default: return 1.0f;
        }
    }

    private float getResolutionForType(int type) {
        switch (type) {
            case TYPE_ACCELEROMETER: return 0.0012f;
            case TYPE_MAGNETIC_FIELD: return 0.15f;
            case TYPE_GYROSCOPE: return 0.001f;
            case TYPE_LIGHT: return 1.0f;
            case TYPE_PROXIMITY: return 5.0f;
            case TYPE_GRAVITY: return 0.0012f;
            case TYPE_LINEAR_ACCELERATION: return 0.0012f;
            default: return 0.01f;
        }
    }
}
