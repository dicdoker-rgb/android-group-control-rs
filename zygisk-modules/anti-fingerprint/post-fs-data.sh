#!/system/bin/sh
# Anti-Fingerprint: 生成伪造的 /proc 文件
# 在 Magisk post-fs-data 阶段执行

FAKE_DIR="/data/local/tmp"

# 1. 伪造 /proc/mounts — 模拟真机的 ext4 挂载
cat > "$FAKE_DIR/fake_mounts" << 'MEOF'
rootfs / rootfs ro,seclabel 0 0
tmpfs /dev tmpfs rw,seclabel,nosuid,relatime,size=3891124k,nr_inodes=972781,mode=755 0 0
devpts /dev/pts devpts rw,seclabel,relatime,mode=600,ptmxmode=000 0 0
proc /proc proc rw,relatime 0 0
sysfs /sys sysfs rw,seclabel,relatime 0 0
selinuxfs /sys/fs/selinux selinuxfs rw,relatime 0 0
none /acct cgroup rw,relatime,cpuacct 0 0
tmpfs /mnt tmpfs rw,seclabel,relatime,size=3891124k,nr_inodes=972781,mode=755,gid=1000 0 0
/dev/block/dm-0 /system ext4 ro,seclabel,relatime,discard 0 0
/dev/block/dm-1 /vendor ext4 ro,seclabel,relatime,discard 0 0
/dev/block/sda17 /data ext4 rw,seclabel,nosuid,nodev,noatime,discard,noauto_da_alloc,data=ordered 0 0
/dev/block/sda6 /cache ext4 rw,seclabel,nosuid,nodev,noatime,discard,data=ordered 0 0
/dev/block/sda5 /persist ext4 rw,seclabel,nosuid,nodev,noatime,discard,data=ordered 0 0
none /config configfs rw,relatime 0 0
adb /dev/usb-ffs/adb functionfs rw,relatime 0 0
/data/media /mnt/runtime/default/emulated sdcardfs rw,nosuid,nodev,relatime,uid=1023,gid=1023,derive=legacy,reserved=20MB 0 0
MEOF

# 2. 伪造 /proc/self/mountinfo — 更详细版本
cat > "$FAKE_DIR/fake_mountinfo" << 'MIEOF'
1 0 253:0 / / ro,seclabel,relatime shared:1 - ext4 /dev/block/dm-0 rw,discard
2 1 0:5 / /dev rw,seclabel,nosuid,relatime shared:2 - tmpfs tmpfs rw,size=3891124k,nr_inodes=972781,mode=755
3 2 0:12 / /dev/pts rw,seclabel,relatime shared:3 - devpts devpts rw,mode=600,ptmxmode=000
4 1 0:3 / /proc rw,relatime shared:4 - proc proc rw
5 1 0:14 / /sys rw,seclabel,relatime shared:5 - sysfs sysfs rw
6 5 0:7 / /sys/fs/selinux rw,relatime shared:6 - selinuxfs selinuxfs rw
7 1 0:15 / /mnt rw,seclabel,relatime shared:7 - tmpfs tmpfs rw,size=3891124k,nr_inodes=972781,mode=755,gid=1000
8 1 253:1 / /vendor ro,seclabel,relatime shared:8 - ext4 /dev/block/dm-1 rw,discard
9 1 8:17 / /data rw,seclabel,nosuid,nodev,noatime shared:9 - ext4 /dev/block/sda17 rw,discard,noauto_da_alloc,data=ordered
MIEOF

# 3. 伪造 /proc/1/cgroup — 模拟真机
cat > "$FAKE_DIR/fake_cgroup" << 'CEOF'
12:cpuset:/
11:cpu,cpuacct:/uid_10000/pid_1234
10:schedtune:/
9:memory:/
8:freezer:/
7:devices:/
6:blkio:/
5:pids:/uid_10000/pid_1234
4:net_cls,net_prio:/
3:cpuctl:/
2:perf_event:/
1:name=systemd:/init.scope
0::/init.scope
CEOF

# 4. 伪造 SELinux enforce 状态
echo -n "1" > "$FAKE_DIR/fake_enforce"

# 5. 删除 Docker 标识
rm -f /.dockerenv 2>/dev/null

# 6. Bind mount SELinux enforce
mount -o bind "$FAKE_DIR/fake_enforce" /sys/fs/selinux/enforce 2>/dev/null

chmod 644 "$FAKE_DIR/fake_mounts"
chmod 644 "$FAKE_DIR/fake_mountinfo"
chmod 644 "$FAKE_DIR/fake_cgroup"
chmod 644 "$FAKE_DIR/fake_enforce"
