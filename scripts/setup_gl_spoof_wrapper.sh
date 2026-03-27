#!/bin/bash
# 从宿主机设置GL spoof wrapper到容器overlay层
# 用法: ./setup_gl_spoof_wrapper.sh <container_name>

CONTAINER=${1:-redroid-v4}
UPPER=$(docker inspect $CONTAINER --format "{{.GraphDriver.Data.UpperDir}}")
MERGED=$(docker inspect $CONTAINER --format "{{.GraphDriver.Data.MergedDir}}")

# 1. 把原始app_process64改名
mkdir -p $UPPER/system/bin
cp $MERGED/system/bin/app_process64 $UPPER/system/bin/app_process64_real

# 2. 创建wrapper
cat > $UPPER/system/bin/app_process64 << 'WRAPPER'
#!/system/bin/sh
export LD_PRELOAD=/system/lib64/libgl_spoof.so
exec /system/bin/app_process64_real "$@"
WRAPPER
chmod 755 $UPPER/system/bin/app_process64
chmod 755 $UPPER/system/bin/app_process64_real

echo "GL spoof wrapper deployed for $CONTAINER"
