set(OBS_FRAMEWORKS_DIR "${OBS_APP_BUNDLE}/Contents/Frameworks")

add_library(obs-telegram-send MODULE src/module.cpp)
target_compile_features(obs-telegram-send PRIVATE cxx_std_17)
target_include_directories(obs-telegram-send PRIVATE "${OBS_INCLUDE_DIR}")
target_link_libraries(obs-telegram-send PRIVATE
  "${OBS_FRAMEWORKS_DIR}/libobs.framework"
  "${OBS_FRAMEWORKS_DIR}/obs-frontend-api.dylib"
)

set_target_properties(obs-telegram-send PROPERTIES
  BUNDLE TRUE
  BUNDLE_EXTENSION plugin
  PREFIX ""
)

if(BUILD_TESTING)
  add_subdirectory(tests)
endif()
