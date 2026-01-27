pluginManagement {
    repositories {
        gradlePluginPortal()
        google()
        mavenCentral()
    }
}

dependencyResolutionManagement {
    repositoriesMode.set(RepositoriesMode.FAIL_ON_PROJECT_REPOS)
    repositories {
        google()
        mavenCentral()
        maven { url = uri("https://jitpack.io") }
        maven {
            url = uri("https://maven.pkg.github.com/jonasfranz/ktor-client-karoo")
            credentials {
                username = providers.gradleProperty("gpruser").getOrElse(System.getenv("USERNAME"))
                password = providers.gradleProperty("gprkey").getOrElse(System.getenv("TOKEN"))
            }
        }
    }
}


rootProject.name = "karoo-rain-radar"
include(":app")
