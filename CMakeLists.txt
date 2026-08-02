cmake_minimum_required(VERSION 3.28)

project(obs-telegram-send VERSION 0.1.0 LANGUAGES CXX)

set(OBS_APP_BUNDLE "/Applications/OBS.app" CACHE PATH "Path to OBS Studio.app")

if(NOT EXISTS "${OBS_APP_BUNDLE}/Contents/Frameworks/libobs.framework")
  message(FATAL_ERROR "OBS_APP_BUNDLE must point to an OBS Studio.app bundle")
endif()

include(CTest)
add_subdirectory(plugin)
