# Karoo extension
-keep class io.github.glandais.karoo.rainradar.extension.** { *; }

# Ktor
-keep class io.ktor.** { *; }
-keepclassmembers class io.ktor.** { *; }
-dontwarn io.ktor.**

# Serialization
-keepattributes *Annotation*, InnerClasses
-dontnote kotlinx.serialization.AnnotationsKt
-keepclassmembers class kotlinx.serialization.json.** {
    *** Companion;
}
-keepclasseswithmembers class kotlinx.serialization.json.** {
    kotlinx.serialization.KSerializer serializer(...);
}
-keep,includedescriptorclasses class io.github.glandais.karoo.rainradar.**$$serializer { *; }
-keepclassmembers class io.github.glandais.karoo.rainradar.** {
    *** Companion;
}
-keepclasseswithmembers class io.github.glandais.karoo.rainradar.** {
    kotlinx.serialization.KSerializer serializer(...);
}
