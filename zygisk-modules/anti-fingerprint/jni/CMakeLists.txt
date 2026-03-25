cmake_minimum_required(VERSION 3.10)
project(anti_fingerprint)

set(CMAKE_CXX_STANDARD 17)

# Dobby inline hook framework
# 需要从 https://github.com/jmpews/Dobby 获取预编译库
# 或者作为子模块编译
set(DOBBY_DIR ${CMAKE_CURRENT_SOURCE_DIR}/dobby)

add_library(anti_fingerprint SHARED
    main.cpp
)

target_include_directories(anti_fingerprint PRIVATE
    ${CMAKE_CURRENT_SOURCE_DIR}
    ${DOBBY_DIR}/include
)

# 链接 Dobby
target_link_libraries(anti_fingerprint
    ${DOBBY_DIR}/libs/${ANDROID_ABI}/libdobby.a
    log
    dl
)

# 输出路径
set_target_properties(anti_fingerprint PROPERTIES
    LIBRARY_OUTPUT_DIRECTORY "${CMAKE_CURRENT_SOURCE_DIR}/../libs/${ANDROID_ABI}"
)
