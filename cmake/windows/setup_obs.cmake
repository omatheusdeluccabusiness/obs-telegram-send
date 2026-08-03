include_guard(GLOBAL)

set(OBS_TELEGRAM_OBS_VERSION "32.1.1")
set(OBS_TELEGRAM_OBS_SOURCE_SHA256 "f4c17e1aa2a00efd8729ed6cfef9308bc6a2b6c583c187d7959eaa72eb2e3676")
set(OBS_TELEGRAM_DEPS_VERSION "2025-08-23")
set(OBS_TELEGRAM_DEPS_SHA256 "8de229cff6f1981508c0eb646b35e644633a5855787b9f5d3b90ae2aeb87ffc1")
set(OBS_TELEGRAM_QT_DEPS_SHA256 "c62e82483bc7c0bf199e8ac3220c66a85a6e8a0cd69a05b6d44f873b830e415f")

set(_obs_telegram_deps_root "${CMAKE_BINARY_DIR}/_obs_telegram_dependencies")
set(_obs_telegram_source_archive "${_obs_telegram_deps_root}/obs-studio.tar.gz")
set(_obs_telegram_deps_archive "${_obs_telegram_deps_root}/obs-deps.zip")
set(_obs_telegram_qt_archive "${_obs_telegram_deps_root}/obs-qt6.zip")
set(_obs_telegram_source "")
set(_obs_telegram_prebuilt "${_obs_telegram_deps_root}/prebuilt")
set(_obs_telegram_qt "${_obs_telegram_deps_root}/qt6")
set(_obs_telegram_install "${_obs_telegram_deps_root}/install")
set(_obs_telegram_build "${_obs_telegram_source}/build_x64")

function(_obs_telegram_download url destination sha256)
  if(EXISTS "${destination}")
    file(SHA256 "${destination}" actual_sha256)
    if(actual_sha256 STREQUAL sha256)
      return()
    endif()
    file(REMOVE "${destination}")
  endif()
  file(DOWNLOAD "${url}" "${destination}" EXPECTED_HASH "SHA256=${sha256}" STATUS result TLS_VERIFY ON)
  list(GET result 0 status_code)
  list(GET result 1 status_message)
  if(NOT status_code EQUAL 0)
    message(FATAL_ERROR "Could not download required OBS dependency: ${status_message}")
  endif()
endfunction()

file(MAKE_DIRECTORY "${_obs_telegram_deps_root}")
_obs_telegram_download(
  "https://github.com/obsproject/obs-studio/releases/download/${OBS_TELEGRAM_OBS_VERSION}/OBS-Studio-${OBS_TELEGRAM_OBS_VERSION}-Sources.tar.gz"
  "${_obs_telegram_source_archive}" "${OBS_TELEGRAM_OBS_SOURCE_SHA256}")
_obs_telegram_download(
  "https://github.com/obsproject/obs-deps/releases/download/${OBS_TELEGRAM_DEPS_VERSION}/windows-deps-${OBS_TELEGRAM_DEPS_VERSION}-x64.zip"
  "${_obs_telegram_deps_archive}" "${OBS_TELEGRAM_DEPS_SHA256}")
_obs_telegram_download(
  "https://github.com/obsproject/obs-deps/releases/download/${OBS_TELEGRAM_DEPS_VERSION}/windows-deps-qt6-${OBS_TELEGRAM_DEPS_VERSION}-x64.zip"
  "${_obs_telegram_qt_archive}" "${OBS_TELEGRAM_QT_DEPS_SHA256}")

file(GLOB _obs_telegram_source_candidates LIST_DIRECTORIES true
  "${_obs_telegram_deps_root}/obs-studio-${OBS_TELEGRAM_OBS_VERSION}*")
foreach(candidate IN LISTS _obs_telegram_source_candidates)
  if(EXISTS "${candidate}/CMakeLists.txt")
    set(_obs_telegram_source "${candidate}")
    break()
  endif()
endforeach()
if(NOT _obs_telegram_source)
  file(ARCHIVE_EXTRACT INPUT "${_obs_telegram_source_archive}" DESTINATION "${_obs_telegram_deps_root}")
  file(GLOB _obs_telegram_source_candidates LIST_DIRECTORIES true
    "${_obs_telegram_deps_root}/obs-studio-${OBS_TELEGRAM_OBS_VERSION}*")
  foreach(candidate IN LISTS _obs_telegram_source_candidates)
    if(EXISTS "${candidate}/CMakeLists.txt")
      set(_obs_telegram_source "${candidate}")
      break()
    endif()
  endforeach()
endif()
if(NOT _obs_telegram_source)
  message(FATAL_ERROR "The verified OBS archive did not contain an OBS source directory.")
endif()
set(OBS_INCLUDE_DIR "${_obs_telegram_source}/libobs" CACHE PATH "Pinned OBS headers" FORCE)
set(OBS_FRONTEND_INCLUDE_DIR "${_obs_telegram_source}/frontend/api" CACHE PATH "Pinned OBS frontend headers" FORCE)
set(OBS_DEPENDENCIES_INCLUDE_DIR "${_obs_telegram_prebuilt}/include" CACHE PATH "Pinned OBS dependency headers" FORCE)
if(NOT EXISTS "${_obs_telegram_prebuilt}/share/obs-deps/VERSION")
  file(MAKE_DIRECTORY "${_obs_telegram_prebuilt}")
  file(ARCHIVE_EXTRACT INPUT "${_obs_telegram_deps_archive}" DESTINATION "${_obs_telegram_prebuilt}")
endif()
if(NOT EXISTS "${_obs_telegram_qt}/lib/cmake/Qt6/Qt6Config.cmake")
  file(MAKE_DIRECTORY "${_obs_telegram_qt}")
  file(ARCHIVE_EXTRACT INPUT "${_obs_telegram_qt_archive}" DESTINATION "${_obs_telegram_qt}")
endif()

if(NOT EXISTS "${_obs_telegram_install}/lib/cmake/libobs/libobsConfig.cmake")
  execute_process(
    COMMAND "${CMAKE_COMMAND}" -S "${_obs_telegram_source}" -B "${_obs_telegram_build}"
      -G "${CMAKE_GENERATOR}" -A x64
      -DOBS_CMAKE_VERSION:STRING=3.0.0
      -DOBS_VERSION_OVERRIDE:STRING=${OBS_TELEGRAM_OBS_VERSION}
      -DENABLE_PLUGINS:BOOL=OFF
      -DENABLE_FRONTEND:BOOL=OFF
      -DCMAKE_ENABLE_SCRIPTING:BOOL=OFF
      "-DCMAKE_PREFIX_PATH=${_obs_telegram_prebuilt};${_obs_telegram_qt}"
    COMMAND_ERROR_IS_FATAL ANY)
  execute_process(
    COMMAND "${CMAKE_COMMAND}" --build "${_obs_telegram_build}" --target obs-frontend-api --config Release --parallel
    COMMAND_ERROR_IS_FATAL ANY)
  execute_process(
    COMMAND "${CMAKE_COMMAND}" --install "${_obs_telegram_build}" --component Development --config Release --prefix "${_obs_telegram_install}"
    COMMAND_ERROR_IS_FATAL ANY)
endif()

list(PREPEND CMAKE_PREFIX_PATH "${_obs_telegram_install}" "${_obs_telegram_qt}" "${_obs_telegram_prebuilt}")
find_package(libobs REQUIRED)
find_package(obs-frontend-api REQUIRED)
find_package(Qt6 REQUIRED COMPONENTS Network Widgets)
