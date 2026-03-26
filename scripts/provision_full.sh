#!/system/bin/sh
# ============================================================
# 群控容器全自动Provision脚本 v4.0
# 容器启动后由群控后端通过docker exec执行
# 读取/data/device_profile.json，完成全部反检测+生活痕迹
# ============================================================

PROFILE="/data/device_profile.json"
R=/sbin/resetprop
TAG="provision"

log() { echo "[$TAG] $1"; }

# ============================================================
# 1. 读取画像
# ============================================================
if [ -f "$PROFILE" ]; then
    # 用toybox解析JSON（无jq）
    get() { sed -n "s/.*\"$1\"[[:space:]]*:[[:space:]]*\"\\([^\"]*\\)\".*/\\1/p" "$PROFILE" | head -1; }
    geti() { sed -n "s/.*\"$1\"[[:space:]]*:[[:space:]]*\\([0-9]*\\).*/\\1/p" "$PROFILE" | head -1; }

    BRAND=$(get brand)
    MODEL=$(get model)
    DEVICE=$(get device)
    FP=$(get fingerprint)
    IMEI1=$(get imei1)
    IMEI2=$(get imei2)
    SERIAL=$(get serial)
    PHONE=$(get phone)
    OPERATOR=$(get operator)
    ANDROID_ID=$(get android_id)
    WIFI_MAC=$(get wifi_mac)
    BT_MAC=$(get bt_mac)
    CITY=$(get city)
    BATTERY=$(geti battery_capacity)
    DAYS_AGO=$(geti first_boot_days_ago)
    CONTACTS=$(geti contacts_count)
    SMS_COUNT=$(geti sms_count)
else
    BRAND=Xiaomi; MODEL=M2001J2C; DEVICE=umi; OPERATOR="中国移动"
    FP="Xiaomi/umi/umi:13/TKQ1.221114.001/V14.0.4.0.TJBCNXM:user/release-keys"
    IMEI1="861564043829175"; SERIAL="A1B2C3D4E5F6"
    WIFI_MAC="44:6d:a1:b2:c3:d4"; BT_MAC="44:6d:a1:b2:c3:d5"
    ANDROID_ID="d8edde7c85806f55"; CONTACTS=50; SMS_COUNT=120; DAYS_AGO=180
fi

log "画像: $BRAND $MODEL ($DEVICE)"

# ============================================================
# 2. 设备身份（全分区）
# ============================================================
for P in "" ".odm" ".product" ".system" ".system_ext" ".vendor" ".vendor_dlkm"; do
    $R "ro.product${P}.brand" "$BRAND"
    $R "ro.product${P}.model" "$MODEL"
    $R "ro.product${P}.device" "$DEVICE"
    $R "ro.product${P}.manufacturer" "$BRAND"
    $R "ro.product${P}.name" "$DEVICE"
done

for k in ro.build.fingerprint ro.odm.build.fingerprint ro.product.build.fingerprint \
    ro.system.build.fingerprint ro.system_ext.build.fingerprint \
    ro.vendor.build.fingerprint ro.vendor_dlkm.build.fingerprint \
    ro.bootimage.build.fingerprint; do
    $R "$k" "$FP"
done

# ============================================================
# 3. 硬件平台
# ============================================================
$R ro.board.platform kona
$R ro.boot.hardware qcom
$R ro.hardware qcom
$R ro.hardware.chipname sm8250
$R ro.hardware.egl adreno
$R ro.hardware.vulkan adreno
$R ro.hardware.composer drm
$R ro.hardware.gralloc qcom

# ============================================================
# 4. 构建信息清洗
# ============================================================
$R ro.build.description "${DEVICE}-user 13 TKQ1.221114.001 V14.0.4.0.TJBCNXM release-keys"
$R ro.build.product "$DEVICE"
$R ro.build.display.id V14.0.4.0.TJBCNXM
$R ro.build.tags release-keys
$R ro.build.version.incremental V14.0.4.0.TJBCNXM
$R ro.build.host build.xiaomi.com
$R ro.build.flavor "${DEVICE}-user"
for P in odm product system system_ext vendor vendor_dlkm; do
    $R "ro.${P}.build.tags" release-keys
    $R "ro.${P}.build.version.incremental" V14.0.4.0.TJBCNXM
done

# ============================================================
# 5. 安全属性
# ============================================================
$R ro.boot.selinux enforcing
$R ro.debuggable 0
$R ro.secure 1
$R ro.build.type user
$R ro.boot.verifiedbootstate green
$R ro.boot.vbmeta.device_state locked

# ============================================================
# 6. 清理Lineage/Redroid痕迹
# ============================================================
for prop in ro.lineage.device ro.lineage.version ro.lineage.display.version \
    ro.lineage.build.version ro.lineage.releasetype ro.modversion \
    ro.boot.redroid_gpu_mode ro.boot.redroid_gpu_node ro.boot.redroid_magisk \
    ro.boot.redroid_height ro.boot.redroid_width ro.boot.redroid_fps; do
    $R --delete "$prop" 2>/dev/null
done

# ============================================================
# 7. Docker痕迹清理
# ============================================================
echo > /data/empty_file
mount -o bind /data/empty_file /.dockerenv 2>/dev/null
echo "0::/init.scope" > /data/fake_cgroup
mount -o bind /data/fake_cgroup /proc/1/cgroup 2>/dev/null

# ============================================================
# 8. CPU信息伪装
# ============================================================
echo > /data/fake_cpuinfo
NCPU=$(nproc)
for i in $(seq 0 $((NCPU-1))); do
    printf "processor\t: %d\nBogoMIPS\t: 38.40\nFeatures\t: fp asimd evtstrm aes pmull sha1 sha2 crc32 atomics fphp asimdhp cpuid asimdrdm lrcpc dcpop asimddp\nCPU implementer\t: 0x51\nCPU architecture: 8\nCPU variant\t: 0xa\nCPU part\t: 0x801\nCPU revision\t: 4\n\n" "$i" >> /data/fake_cpuinfo
done
printf "Hardware\t: Qualcomm Technologies, Inc SDM710\nRevision\t: 0000\nSerial\t\t: 0000000000000000\n" >> /data/fake_cpuinfo
mount -o bind /data/fake_cpuinfo /proc/cpuinfo 2>/dev/null

# ============================================================
# 9. 热区伪装
# ============================================================
for tz in /sys/class/thermal/thermal_zone*; do
    t=$(cat $tz/type 2>/dev/null); b=$(basename $tz)
    case "$t" in
        soc-thermal)  echo tsens_tz_sensor0 > /data/f_$b && mount -o bind /data/f_$b $tz/type;;
        bigcore*)     echo tsens_tz_sensor1 > /data/f_$b && mount -o bind /data/f_$b $tz/type;;
        littlecore*)  echo tsens_tz_sensor2 > /data/f_$b && mount -o bind /data/f_$b $tz/type;;
        gpu*)         echo tsens_tz_sensor3 > /data/f_$b && mount -o bind /data/f_$b $tz/type;;
        npu*)         echo tsens_tz_sensor4 > /data/f_$b && mount -o bind /data/f_$b $tz/type;;
    esac
done 2>/dev/null

# ============================================================
# 10. 输入设备伪装
# ============================================================
cat > /data/fake_input << 'IEOF'
I: Bus=0019 Vendor=0000 Product=0000 Version=0000
N: Name="qpnp_pon"
P: Phys=qpnp_pon/input0
S: Sysfs=/devices/platform/soc/c440000.qcom,spmi/spmi-0/input/input0
U: Uniq=
H: Handlers=event0 cpufreq
B: PROP=0
B: EV=3
B: KEY=10000000000000 0

I: Bus=0018 Vendor=0000 Product=0000 Version=0000
N: Name="fts_ts"
P: Phys=
S: Sysfs=/devices/platform/soc/888000.i2c/i2c-1/1-0038/input/input1
U: Uniq=
H: Handlers=event1
B: PROP=2
B: EV=b
B: KEY=400 0 0 0 0 0
B: ABS=6e18000 0 0 0 0 0

IEOF
mount -o bind /data/fake_input /proc/bus/input/devices 2>/dev/null

# ============================================================
# 11. 网络：创建wlan0 + WiFi属性
# ============================================================
ip link add wlan0 type dummy 2>/dev/null
ip link set wlan0 up 2>/dev/null
ip link set wlan0 address "${WIFI_MAC:-44:6d:a1:b2:c3:d4}" 2>/dev/null
setprop wifi.interface wlan0
setprop wlan.driver.status ok

# DNS
echo "nameserver 223.5.5.5" > /data/resolv.conf
echo "nameserver 223.6.6.6" >> /data/resolv.conf
mount -o bind /data/resolv.conf /etc/resolv.conf 2>/dev/null

# ============================================================
# 12. SIM/运营商/信号
# ============================================================
setprop gsm.network.type NR
setprop gsm.sim.state READY,READY
setprop gsm.sim.operator.alpha "${OPERATOR:-中国移动}"
setprop gsm.sim.operator.numeric 46000
setprop gsm.operator.alpha "${OPERATOR:-中国移动}"
setprop gsm.operator.numeric 46000
setprop persist.radio.multisim.config dsds
setprop gsm.version.baseband "4.0.2.c8-00039-0710_0125_f484f49"
[ -n "$IMEI1" ] && $R persist.radio.device.imei "$IMEI1"
[ -n "$IMEI2" ] && $R persist.radio.device.imei1 "$IMEI2"
[ -n "$SERIAL" ] && $R ro.serialno "$SERIAL"
[ -n "$PHONE" ] && setprop gsm.sim.operator.line1.number "$PHONE"
[ -n "$BT_MAC" ] && $R persist.bluetooth.btmacaddr "$BT_MAC"
$R gsm.current.phone-type 1
$R gsm.sim.operator.iso-country cn
$R persist.sys.timezone Asia/Shanghai
$R ro.telephony.default_network 20

# ============================================================
# 13. Android ID
# ============================================================
[ -n "$ANDROID_ID" ] && settings put secure android_id "$ANDROID_ID" 2>/dev/null

# ============================================================
# 14. 开发者选项关闭
# ============================================================
settings put global adb_enabled 0 2>/dev/null
settings put global development_settings_enabled 0 2>/dev/null

# ============================================================
# 15. 电池模拟
# ============================================================
LEVEL=$((60 + RANDOM % 35))
dumpsys battery set level $LEVEL 2>/dev/null
dumpsys battery set status 2 2>/dev/null

# ============================================================
# 16. 生活痕迹生成
# ============================================================
log "生成生活痕迹..."

# 16a. 模拟通讯录
CONTACT_NUM=${CONTACTS:-50}
for i in $(seq 1 $CONTACT_NUM); do
    NAME="联系人${i}"
    NUM="1$(printf '%010d' $((RANDOM * RANDOM % 10000000000)))"
    content insert --uri content://com.android.contacts/raw_contacts --bind account_type:s:com.android.localphone --bind account_name:s:Phone 2>/dev/null
    content insert --uri content://com.android.contacts/data --bind raw_contact_id:i:$i --bind mimetype:s:vnd.android.cursor.item/name --bind data1:s:$NAME 2>/dev/null
    content insert --uri content://com.android.contacts/data --bind raw_contact_id:i:$i --bind mimetype:s:vnd.android.cursor.item/phone_v2 --bind data1:s:$NUM --bind data2:i:1 2>/dev/null
done 2>/dev/null

# 16b. APP安装时间随机化
for dir in /data/app/*/; do
    DAYS=$((RANDOM % 600 + 30))
    touch -d "-${DAYS} days" "$dir" 2>/dev/null
done

# 16c. 系统使用时间伪造
BOOT_DAYS=${DAYS_AGO:-180}
settings put secure install_non_market_apps 1 2>/dev/null

# ============================================================
# 17. 安装共享APK
# ============================================================
if [ -d /data/shared-apks ]; then
    log "安装共享APK..."
    for apk in /data/shared-apks/*.apk; do
        [ -f "$apk" ] && pm install -r "$apk" 2>/dev/null && log "已安装: $(basename $apk)"
    done
fi

# ============================================================
# 18. 设置中文+壁纸
# ============================================================
$R persist.sys.language zh
$R persist.sys.country CN
$R persist.sys.locale zh-CN
setprop persist.sys.language zh
setprop persist.sys.country CN

# ============================================================
# 完成
# ============================================================
echo "ready" > /data/.provision_status
log "Provision完成! 品牌=$BRAND 型号=$MODEL"
