set(OBS_FRAMEWORKS_DIR "${OBS_APP_BUNDLE}/Contents/Frameworks")

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

find_package(Qt6 REQUIRED COMPONENTS Widgets)

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
  src/module.cpp
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
  Qt6::Widgets
  "${OBS_FRAMEWORKS_DIR}/obs-frontend-api.dylib"
)
target_link_options(obs-telegram-send PRIVATE
  "LINKER:${OBS_FRAMEWORKS_DIR}/libobs.framework/Versions/A/libobs"
)

set_target_properties(obs-telegram-send PROPERTIES
  BUNDLE TRUE
  BUNDLE_EXTENSION plugin
  PREFIX ""
)

if(BUILD_TESTING)
  add_subdirectory(tests)
endif()
