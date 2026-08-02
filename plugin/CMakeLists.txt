if(APPLE)
  set(OBS_FRAMEWORKS_DIR "${OBS_APP_BUNDLE}/Contents/Frameworks")
endif()

if(NOT OBS_FRONTEND_INCLUDE_DIR)
  get_filename_component(OBS_SOURCE_DIR "${OBS_INCLUDE_DIR}" DIRECTORY)
  find_path(OBS_FRONTEND_INCLUDE_DIR
    NAMES obs-frontend-api.h
    PATHS "${OBS_SOURCE_DIR}/frontend/api"
    NO_DEFAULT_PATH
  )
endif()
if(NOT EXISTS "${OBS_FRONTEND_INCLUDE_DIR}/obs-frontend-api.h")
  message(FATAL_ERROR
    "OBS_FRONTEND_INCLUDE_DIR must contain the OBS 32.1.1 frontend API header."
  )
endif()

if(NOT OBS_DEPENDENCIES_INCLUDE_DIR)
  find_path(OBS_DEPENDENCIES_INCLUDE_DIR NAMES simde/x86/sse2.h)
endif()
if(NOT EXISTS "${OBS_DEPENDENCIES_INCLUDE_DIR}/simde/x86/sse2.h")
  message(FATAL_ERROR
    "OBS_DEPENDENCIES_INCLUDE_DIR must contain the OBS 32.1.1 dependency headers."
  )
endif()

find_package(Qt6 REQUIRED COMPONENTS Network Widgets)

# OBS source distributions generate this header while building libobs. Plugin
# builds against the matching source headers need the same small config shim.
set(OBS_RELEASE_CANDIDATE 0)
set(OBS_BETA 0)
configure_file(
  "${OBS_INCLUDE_DIR}/obsconfig.h.in"
  "${CMAKE_BINARY_DIR}/generated/obsconfig.h"
  @ONLY
)

add_library(obs-telegram-send MODULE
  src/agent_client.cpp
  src/module.cpp
  src/onboarding_dialog.cpp
  src/recording_controller.cpp
  src/send_confirmation_dialog.cpp
)
target_compile_features(obs-telegram-send PRIVATE cxx_std_17)
target_include_directories(obs-telegram-send PRIVATE
  "${OBS_INCLUDE_DIR}"
  "${OBS_FRONTEND_INCLUDE_DIR}"
  "${OBS_DEPENDENCIES_INCLUDE_DIR}"
  "${CMAKE_BINARY_DIR}/generated"
)
target_link_libraries(obs-telegram-send PRIVATE
  Qt6::Network
  Qt6::Widgets
)
if(APPLE)
  target_link_libraries(obs-telegram-send PRIVATE
    "${OBS_FRAMEWORKS_DIR}/obs-frontend-api.dylib"
  )
  target_link_options(obs-telegram-send PRIVATE
    "LINKER:${OBS_FRAMEWORKS_DIR}/libobs.framework/Versions/A/libobs"
  )
elseif(WIN32)
  target_link_libraries(obs-telegram-send PRIVATE
    "${OBS_WINDOWS_LIBOBS_LIBRARY}"
    "${OBS_WINDOWS_FRONTEND_LIBRARY}"
  )
else()
  message(FATAL_ERROR "Unsupported platform")
endif()

if(APPLE)
  set_target_properties(obs-telegram-send PROPERTIES
    BUNDLE TRUE
    BUNDLE_EXTENSION plugin
    PREFIX ""
  )
elseif(WIN32)
  set_target_properties(obs-telegram-send PROPERTIES PREFIX "")
endif()

if(BUILD_TESTING)
  add_subdirectory(tests)
endif()
