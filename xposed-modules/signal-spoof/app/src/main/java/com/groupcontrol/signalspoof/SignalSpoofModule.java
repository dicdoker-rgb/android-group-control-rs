package com.groupcontrol.signalspoof;

import android.os.SystemProperties;
import android.telephony.CellIdentityLte;
import android.telephony.CellInfo;
import android.telephony.CellInfoLte;
import android.telephony.CellSignalStrengthLte;
import android.telephony.SignalStrength;
import android.telephony.TelephonyManager;

import java.lang.reflect.Constructor;
import java.lang.reflect.Method;
import java.util.ArrayList;
import java.util.List;
import java.util.Random;

import de.robv.android.xposed.IXposedHookLoadPackage;
import de.robv.android.xposed.XC_MethodHook;
import de.robv.android.xposed.XposedBridge;
import de.robv.android.xposed.XposedHelpers;
import de.robv.android.xposed.callbacks.XC_LoadPackage;

public class SignalSpoofModule implements IXposedHookLoadPackage {

    private static final String TAG = "SignalSpoof";
    private final Random random = new Random();

    // Realistic baseband strings per platform
    private static final String[] BASEBAND_STRINGS = {
            "g]]]].F-2.0.c6.0",                          // Qualcomm SDM845
            "M8998-030712-0810191757",                     // Qualcomm MSM8998
            "MPSS.AT.4.0.c2-01162-SDM845_GEN_PACK-2",     // Qualcomm SDM845
            "MPSS.HE.2.0.c1.9-00028-SDM710_GEN_PACK-1",   // Qualcomm SDM710
            "MPSS.AT.2.0.1.c1-00050-0.150310.1",           // Qualcomm MSM8996
            "SAMSUNG_SM-G960F_20R_CL17694214_QB30216019_REV00", // Samsung Exynos
            "21091116AC_MOLY_LR12A_R3_MP_V112.23",         // MediaTek
    };

    @Override
    public void handleLoadPackage(XC_LoadPackage.LoadPackageParam lpparam) throws Throwable {
        XposedBridge.log(TAG + ": Loaded in " + lpparam.packageName);

        hookSystemProperties(lpparam);
        hookGetAllCellInfo(lpparam);
        hookGetSignalStrength(lpparam);
        hookGetSimState(lpparam);
        hookGetLine1Number(lpparam);
        hookGetSubscriberId(lpparam);
        hookGetDeviceId(lpparam);
    }

    /**
     * Hook 1: SystemProperties.get() — spoof baseband version
     */
    private void hookSystemProperties(XC_LoadPackage.LoadPackageParam lpparam) {
        try {
            XposedHelpers.findAndHookMethod(
                    "android.os.SystemProperties",
                    lpparam.classLoader,
                    "get",
                    String.class, String.class,
                    new XC_MethodHook() {
                        @Override
                        protected void beforeHookedMethod(MethodHookParam param) throws Throwable {
                            try {
                                String key = (String) param.args[0];
                                if ("gsm.version.baseband".equals(key)) {
                                    // Pick a deterministic but realistic baseband string
                                    // Use device android_id hash for consistency across reboots
                                    int index = Math.abs(android.os.Build.DEVICE.hashCode()) % BASEBAND_STRINGS.length;
                                    param.setResult(BASEBAND_STRINGS[index]);
                                }
                            } catch (Throwable t) {
                                XposedBridge.log(TAG + ": Error in SystemProperties hook: " + t.getMessage());
                            }
                        }
                    }
            );
            XposedBridge.log(TAG + ": Hooked SystemProperties.get");
        } catch (Throwable t) {
            XposedBridge.log(TAG + ": Failed to hook SystemProperties.get: " + t.getMessage());
        }
    }

    /**
     * Hook 2: TelephonyManager.getAllCellInfo() — return fake LTE CellInfo
     */
    private void hookGetAllCellInfo(XC_LoadPackage.LoadPackageParam lpparam) {
        try {
            XposedHelpers.findAndHookMethod(
                    "android.telephony.TelephonyManager",
                    lpparam.classLoader,
                    "getAllCellInfo",
                    new XC_MethodHook() {
                        @Override
                        protected void afterHookedMethod(MethodHookParam param) throws Throwable {
                            try {
                                List<CellInfo> fakeCellInfoList = new ArrayList<>();

                                // Create a CellInfoLte via reflection (constructor not public)
                                Class<?> cellInfoLteClass = Class.forName("android.telephony.CellInfoLte");
                                CellInfoLte cellInfoLte = (CellInfoLte) cellInfoLteClass.getDeclaredConstructor().newInstance();

                                // Random signal strength between -85 and -105 dBm
                                int rsrp = -(85 + random.nextInt(21)); // -85 to -105
                                int rsrq = -(3 + random.nextInt(8));   // -3 to -10
                                int rssnr = 10 + random.nextInt(20);   // 10 to 29
                                int cqi = 5 + random.nextInt(10);      // 5 to 14
                                int timingAdvance = Integer.MAX_VALUE;

                                // Set CellSignalStrengthLte
                                CellSignalStrengthLte signalStrength = (CellSignalStrengthLte)
                                        XposedHelpers.callMethod(cellInfoLte, "getCellSignalStrength");

                                // Use reflection to set internal fields
                                XposedHelpers.setIntField(signalStrength, "mRsrp", rsrp);
                                XposedHelpers.setIntField(signalStrength, "mRsrq", rsrq);
                                XposedHelpers.setIntField(signalStrength, "mRssnr", rssnr);
                                XposedHelpers.setIntField(signalStrength, "mCqi", cqi);
                                XposedHelpers.setIntField(signalStrength, "mTimingAdvance", timingAdvance);

                                // Set CellIdentityLte with realistic values
                                CellIdentityLte cellIdentity = (CellIdentityLte)
                                        XposedHelpers.callMethod(cellInfoLte, "getCellIdentity");

                                // MCC=460, MNC=00 (China Mobile), CI and PCI random
                                XposedHelpers.setIntField(cellIdentity, "mMcc", 460);
                                XposedHelpers.setIntField(cellIdentity, "mMnc", 0);
                                XposedHelpers.setIntField(cellIdentity, "mCi", 100000 + random.nextInt(900000));
                                XposedHelpers.setIntField(cellIdentity, "mPci", random.nextInt(504));
                                XposedHelpers.setIntField(cellIdentity, "mTac", 10000 + random.nextInt(50000));
                                XposedHelpers.setIntField(cellIdentity, "mEarfcn", 1300 + random.nextInt(200));

                                XposedHelpers.callMethod(cellInfoLte, "setRegistered", true);

                                fakeCellInfoList.add(cellInfoLte);
                                param.setResult(fakeCellInfoList);

                            } catch (Throwable t) {
                                XposedBridge.log(TAG + ": Error in getAllCellInfo hook: " + t.getMessage());
                            }
                        }
                    }
            );
            XposedBridge.log(TAG + ": Hooked TelephonyManager.getAllCellInfo");
        } catch (Throwable t) {
            XposedBridge.log(TAG + ": Failed to hook getAllCellInfo: " + t.getMessage());
        }
    }

    /**
     * Hook 3: TelephonyManager.getSignalStrength() — fake SignalStrength with LTE level 2-4
     */
    private void hookGetSignalStrength(XC_LoadPackage.LoadPackageParam lpparam) {
        try {
            XposedHelpers.findAndHookMethod(
                    "android.telephony.TelephonyManager",
                    lpparam.classLoader,
                    "getSignalStrength",
                    new XC_MethodHook() {
                        @Override
                        protected void afterHookedMethod(MethodHookParam param) throws Throwable {
                            try {
                                Object signalStrength = param.getResult();
                                if (signalStrength == null) {
                                    // Create a new SignalStrength via reflection
                                    Class<?> ssClass = XposedHelpers.findClass(
                                            "android.telephony.SignalStrength", lpparam.classLoader);
                                    signalStrength = ssClass.getDeclaredConstructor().newInstance();
                                }

                                // Random dBm between -85 and -105
                                int rsrp = -(85 + random.nextInt(21));
                                int rsrq = -(3 + random.nextInt(8));
                                int rssnr = 10 + random.nextInt(20);
                                int cqi = 5 + random.nextInt(10);

                                // Determine level (2-4) based on rsrp
                                // Level 4: >= -85, Level 3: >= -95, Level 2: >= -105
                                int level;
                                if (rsrp >= -85) {
                                    level = 4;
                                } else if (rsrp >= -95) {
                                    level = 3;
                                } else {
                                    level = 2;
                                }

                                // Access the internal LTE signal strength object
                                try {
                                    Object lteSignalStrength = XposedHelpers.getObjectField(
                                            signalStrength, "mLte");
                                    if (lteSignalStrength != null) {
                                        XposedHelpers.setIntField(lteSignalStrength, "mRsrp", rsrp);
                                        XposedHelpers.setIntField(lteSignalStrength, "mRsrq", rsrq);
                                        XposedHelpers.setIntField(lteSignalStrength, "mRssnr", rssnr);
                                        XposedHelpers.setIntField(lteSignalStrength, "mCqi", cqi);
                                        XposedHelpers.setIntField(lteSignalStrength, "mLevel", level);
                                    }
                                } catch (Throwable inner) {
                                    // Fallback: try setting fields directly on older API
                                    XposedHelpers.setIntField(signalStrength, "mLteRsrp", rsrp);
                                    XposedHelpers.setIntField(signalStrength, "mLteRsrq", rsrq);
                                    XposedHelpers.setIntField(signalStrength, "mLteRssnr", rssnr);
                                    XposedHelpers.setIntField(signalStrength, "mLteCqi", cqi);
                                }

                                param.setResult(signalStrength);
                            } catch (Throwable t) {
                                XposedBridge.log(TAG + ": Error in getSignalStrength hook: " + t.getMessage());
                            }
                        }
                    }
            );
            XposedBridge.log(TAG + ": Hooked TelephonyManager.getSignalStrength");
        } catch (Throwable t) {
            XposedBridge.log(TAG + ": Failed to hook getSignalStrength: " + t.getMessage());
        }
    }

    /**
     * Hook 4: TelephonyManager.getSimState() — always return SIM_STATE_READY
     */
    private void hookGetSimState(XC_LoadPackage.LoadPackageParam lpparam) {
        try {
            // Hook the no-arg version
            XposedHelpers.findAndHookMethod(
                    "android.telephony.TelephonyManager",
                    lpparam.classLoader,
                    "getSimState",
                    new XC_MethodHook() {
                        @Override
                        protected void afterHookedMethod(MethodHookParam param) throws Throwable {
                            try {
                                param.setResult(TelephonyManager.SIM_STATE_READY);
                            } catch (Throwable t) {
                                XposedBridge.log(TAG + ": Error in getSimState hook: " + t.getMessage());
                            }
                        }
                    }
            );

            // Also hook the version with slot index
            XposedHelpers.findAndHookMethod(
                    "android.telephony.TelephonyManager",
                    lpparam.classLoader,
                    "getSimState",
                    int.class,
                    new XC_MethodHook() {
                        @Override
                        protected void afterHookedMethod(MethodHookParam param) throws Throwable {
                            try {
                                param.setResult(TelephonyManager.SIM_STATE_READY);
                            } catch (Throwable t) {
                                XposedBridge.log(TAG + ": Error in getSimState(int) hook: " + t.getMessage());
                            }
                        }
                    }
            );
            XposedBridge.log(TAG + ": Hooked TelephonyManager.getSimState");
        } catch (Throwable t) {
            XposedBridge.log(TAG + ": Failed to hook getSimState: " + t.getMessage());
        }
    }

    /**
     * Hook 5: TelephonyManager.getLine1Number() — return phone number from system property
     */
    private void hookGetLine1Number(XC_LoadPackage.LoadPackageParam lpparam) {
        try {
            XposedHelpers.findAndHookMethod(
                    "android.telephony.TelephonyManager",
                    lpparam.classLoader,
                    "getLine1Number",
                    new XC_MethodHook() {
                        @Override
                        protected void afterHookedMethod(MethodHookParam param) throws Throwable {
                            try {
                                // Read phone number from system property set by group control
                                String phoneNumber = (String) XposedHelpers.callStaticMethod(
                                        XposedHelpers.findClass("android.os.SystemProperties", lpparam.classLoader),
                                        "get", "gsm.sim.line1.number", "");
                                if (phoneNumber != null && !phoneNumber.isEmpty()) {
                                    param.setResult(phoneNumber);
                                }
                            } catch (Throwable t) {
                                XposedBridge.log(TAG + ": Error in getLine1Number hook: " + t.getMessage());
                            }
                        }
                    }
            );
            XposedBridge.log(TAG + ": Hooked TelephonyManager.getLine1Number");
        } catch (Throwable t) {
            XposedBridge.log(TAG + ": Failed to hook getLine1Number: " + t.getMessage());
        }
    }

    /**
     * Hook 6: TelephonyManager.getSubscriberId() — return fake IMSI
     */
    private void hookGetSubscriberId(XC_LoadPackage.LoadPackageParam lpparam) {
        try {
            XposedHelpers.findAndHookMethod(
                    "android.telephony.TelephonyManager",
                    lpparam.classLoader,
                    "getSubscriberId",
                    new XC_MethodHook() {
                        @Override
                        protected void afterHookedMethod(MethodHookParam param) throws Throwable {
                            try {
                                // Generate a realistic IMSI: MCC(460) + MNC(00) + MSIN(10 digits)
                                // Use device-based seed for consistency
                                Random deviceRandom = new Random(android.os.Build.SERIAL.hashCode());
                                StringBuilder imsi = new StringBuilder("46000"); // China Mobile
                                for (int i = 0; i < 10; i++) {
                                    imsi.append(deviceRandom.nextInt(10));
                                }
                                param.setResult(imsi.toString());
                            } catch (Throwable t) {
                                XposedBridge.log(TAG + ": Error in getSubscriberId hook: " + t.getMessage());
                            }
                        }
                    }
            );
            XposedBridge.log(TAG + ": Hooked TelephonyManager.getSubscriberId");
        } catch (Throwable t) {
            XposedBridge.log(TAG + ": Failed to hook getSubscriberId: " + t.getMessage());
        }
    }

    /**
     * Hook 7: TelephonyManager.getDeviceId() — return IMEI from system property
     */
    private void hookGetDeviceId(XC_LoadPackage.LoadPackageParam lpparam) {
        try {
            // Hook no-arg version
            XposedHelpers.findAndHookMethod(
                    "android.telephony.TelephonyManager",
                    lpparam.classLoader,
                    "getDeviceId",
                    new XC_MethodHook() {
                        @Override
                        protected void afterHookedMethod(MethodHookParam param) throws Throwable {
                            try {
                                String imei = (String) XposedHelpers.callStaticMethod(
                                        XposedHelpers.findClass("android.os.SystemProperties", lpparam.classLoader),
                                        "get", "persist.radio.device.imei", "");
                                if (imei != null && !imei.isEmpty()) {
                                    param.setResult(imei);
                                }
                            } catch (Throwable t) {
                                XposedBridge.log(TAG + ": Error in getDeviceId hook: " + t.getMessage());
                            }
                        }
                    }
            );

            // Hook version with slot index
            XposedHelpers.findAndHookMethod(
                    "android.telephony.TelephonyManager",
                    lpparam.classLoader,
                    "getDeviceId",
                    int.class,
                    new XC_MethodHook() {
                        @Override
                        protected void afterHookedMethod(MethodHookParam param) throws Throwable {
                            try {
                                String imei = (String) XposedHelpers.callStaticMethod(
                                        XposedHelpers.findClass("android.os.SystemProperties", lpparam.classLoader),
                                        "get", "persist.radio.device.imei", "");
                                if (imei != null && !imei.isEmpty()) {
                                    param.setResult(imei);
                                }
                            } catch (Throwable t) {
                                XposedBridge.log(TAG + ": Error in getDeviceId(int) hook: " + t.getMessage());
                            }
                        }
                    }
            );
            XposedBridge.log(TAG + ": Hooked TelephonyManager.getDeviceId");
        } catch (Throwable t) {
            XposedBridge.log(TAG + ": Failed to hook getDeviceId: " + t.getMessage());
        }
    }
}
