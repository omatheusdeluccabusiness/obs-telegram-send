cmake_minimum_required(VERSION 3.28)

project(obs-telegram-send VERSION 0.1.0 LANGUAGES CXX)

set(OBS_APP_BUNDLE "/Applications/OBS.app" CACHE PATH "Path to OBS Studio.app (macOS only)")
set(OBS_INCLUDE_DIR "" CACHE PATH "Directory containing OBS 32.1.1 obs-module.h")
set(OBS_WINDOWS_LIBOBS_LIBRARY "" CACHE FILEPATH "Path to obs.lib (Windows only)")
set(OBS_WINDOWS_FRONTEND_LIBRARY "" CACHE FILEPATH "Path to obs-frontend-api.lib (Windows only)")

if(APPLE AND NOT EXISTS "${OBS_APP_BUNDLE}/Contents/Frameworks/libobs.framework")
  message(FATAL_ERROR "OBS_APP_BUNDLE must point to an OBS Studio.app bundle")
endif()

if(WIN32)
  if(NOT EXISTS "${OBS_WINDOWS_LIBOBS_LIBRARY}")
    message(FATAL_ERROR "OBS_WINDOWS_LIBOBS_LIBRARY must point to obs.lib from OBS 32.1.1 x64")
  endif()
  if(NOT EXISTS "${OBS_WINDOWS_FRONTEND_LIBRARY}")
    message(FATAL_ERROR "OBS_WINDOWS_FRONTEND_LIBRARY must point to obs-frontend-api.lib from OBS 32.1.1 x64")
  endif()
endif()

if(NOT OBS_INCLUDE_DIR)
  find_path(OBS_INCLUDE_DIR
    NAMES obs-module.h
    PATHS
      "${OBS_APP_BUNDLE}/Contents/include"
      "/opt/homebrew/include"
      "/usr/local/include"
    PATH_SUFFIXES obs
  )
endif()

if(NOT EXISTS "${OBS_INCLUDE_DIR}/obs-module.h")
  message(FATAL_ERROR
    "OBS_INCLUDE_DIR must contain obs-module.h from OBS Studio 32.1.1. "
    "Pass -DOBS_INCLUDE_DIR=/path/to/obs-headers."
  )
endif()

include(CTest)
add_subdirectory(plugin)

if(APPLE)
  set(OBS_TELEGRAM_PLUGIN_INSTALL_DIR
      "Library/Application Support/obs-studio/plugins"
      CACHE PATH "System OBS plugin directory used by the macOS package")
elseif(WIN32)
  set(OBS_TELEGRAM_PLUGIN_INSTALL_DIR
      "obs-telegram-send/bin/64bit"
      CACHE PATH "Relative user plugin directory used by the Windows package")
else()
  message(FATAL_ERROR "OBS Telegram Send currently supports macOS and Windows only")
endif()
install(TARGETS obs-telegram-send
  BUNDLE DESTINATION "${OBS_TELEGRAM_PLUGIN_INSTALL_DIR}"
  LIBRARY DESTINATION "${OBS_TELEGRAM_PLUGIN_INSTALL_DIR}"
  RUNTIME DESTINATION "${OBS_TELEGRAM_PLUGIN_INSTALL_DIR}")
