include(FetchContent)

FetchContent_Declare(
  googletest
  URL https://github.com/google/googletest/archive/refs/tags/v1.17.0.tar.gz
  DOWNLOAD_EXTRACT_TIMESTAMP TRUE
)
set(INSTALL_GTEST OFF CACHE BOOL "" FORCE)
FetchContent_MakeAvailable(googletest)

add_executable(plugin-tests
  recording_controller_test.cpp
  ../src/recording_controller.cpp
  ../src/send_confirmation_dialog.cpp
)
target_compile_features(plugin-tests PRIVATE cxx_std_17)
target_include_directories(plugin-tests PRIVATE
  "${PROJECT_SOURCE_DIR}/plugin/src"
  "${OBS_INCLUDE_DIR}"
  "${OBS_FRONTEND_INCLUDE_DIR}"
  "${OBS_DEPENDENCIES_INCLUDE_DIR}"
  "${CMAKE_BINARY_DIR}/generated"
)
target_link_libraries(plugin-tests PRIVATE GTest::gtest Qt6::Widgets)

include(GoogleTest)
gtest_discover_tests(plugin-tests)
