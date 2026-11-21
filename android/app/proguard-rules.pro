# Keep model classes
-keep class app.clipdeck.desktop.data.** { *; }
-keep class app.clipdeck.desktop.** { *; }
-dontwarn android.**

# Jackson + Kotlin: the release build must keep the metadata Jackson reads at runtime.
-keepattributes *Annotation*, Signature, InnerClasses, EnclosingMethod, RuntimeVisibleAnnotations
-keep class kotlin.Metadata { *; }
-keep class kotlin.reflect.** { *; }
-keepclassmembers class * { @com.fasterxml.jackson.annotation.* <fields>; @com.fasterxml.jackson.annotation.* <methods>; }
-keep class com.fasterxml.jackson.module.kotlin.** { *; }
-dontwarn com.fasterxml.jackson.databind.ext.**
-dontwarn java.beans.**
-dontwarn javax.lang.model.**
