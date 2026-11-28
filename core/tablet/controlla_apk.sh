#!/bin/sh
# Controlla che il programma del tablet, una volta installato, si carichi
# (D456).
#
#   sh core/tablet/controlla_apk.sh percorso/dell.apk
#
# Un APK che si costruisce puo' non partire: la sua libreria chiede ad
# Android, all'avvio, ogni simbolo che non ha dentro, e se Android non ce
# l'ha il programma cade prima di mostrare qualcosa («cannot locate
# symbol»). Costruire non lo dice. Il 28 novembre e' successo col microfono
# (D455): Oboe e' C++, la libreria del C++ non era collegata, e sul tablet
# che e' solo NOVA il programma installato non partiva.
#
# Qui si guarda dentro l'APK: per ogni libreria, ogni simbolo che chiede e
# che non e' «debole» deve darlo una delle librerie di Android da cui dice
# di dipendere, com'e' nell'NDK per la versione minima di Android del
# programma. Si lancia prima di `adb install`.
#
# Serve: NDK_HOME (o ANDROID_NDK_HOME), unzip.

set -eu

apk="${1:-}"
if [ -z "$apk" ] || [ ! -f "$apk" ]; then
  echo "uso: sh controlla_apk.sh percorso/dell.apk" >&2
  exit 2
fi
ndk="${NDK_HOME:-${ANDROID_NDK_HOME:-}}"
attrezzi=$(ls -d "$ndk"/toolchains/llvm/prebuilt/*/ 2>/dev/null | head -1)
if [ -z "$ndk" ] || [ -z "$attrezzi" ]; then
  echo "manca l'NDK: NDK_HOME deve dire dov'e'" >&2
  exit 2
fi
# La versione minima di Android del programma, dal progetto.
qui=$(dirname "$0")
minima=$(sed -n 's/.*minSdk *= *\([0-9][0-9]*\).*/\1/p' "$qui/gen/android/app/build.gradle.kts" | head -1)
minima="${minima:-24}"

lavoro=$(mktemp -d)
trap 'rm -rf "$lavoro"' EXIT
unzip -q -o "$apk" 'lib/*' -d "$lavoro"

guai=0
viste=0
for libreria in "$lavoro"/lib/*/*.so; do
  [ -f "$libreria" ] || continue
  viste=$((viste + 1))
  case "$(basename "$(dirname "$libreria")")" in
    arm64-v8a) terna=aarch64-linux-android ;;
    armeabi-v7a) terna=arm-linux-androideabi ;;
    x86_64) terna=x86_64-linux-android ;;
    x86) terna=i686-linux-android ;;
    *) echo "non so che macchina e' $(dirname "$libreria")" >&2; exit 2 ;;
  esac
  di_android="$attrezzi/sysroot/usr/lib/$terna/$minima"
  : > "$lavoro/dati"
  for da in $("$attrezzi/bin/llvm-readelf" -d "$libreria" | sed -n 's/.*(NEEDED).*\[\(.*\)\]/\1/p'); do
    if [ -f "$di_android/$da" ]; then
      "$attrezzi/bin/llvm-nm" -D --defined-only "$di_android/$da" | awk '{print $NF}' | sed 's/@.*//' >> "$lavoro/dati"
    else
      echo "$(basename "$libreria") dipende da $da, che Android $minima non ha" >&2
      guai=$((guai + 1))
    fi
  done
  sort -u "$lavoro/dati" > "$lavoro/dati.ordinati"
  # I simboli deboli («w», «v») possono mancare: chi li usa guarda se ci sono.
  "$attrezzi/bin/llvm-nm" -D --undefined-only "$libreria" | awk '$1 == "U" {print $2}' | sed 's/@.*//' \
    | sort -u > "$lavoro/chiesti"
  mancano=$(comm -23 "$lavoro/chiesti" "$lavoro/dati.ordinati")
  if [ -n "$mancano" ]; then
    echo "$(basename "$libreria") ($terna) chiede simboli che Android $minima non da':" >&2
    echo "$mancano" | sed 's/^/  /' >&2
    guai=$((guai + 1))
  else
    echo "$(basename "$libreria") ($terna): $(wc -l < "$lavoro/chiesti" | tr -d ' ') simboli chiesti, tutti di Android $minima"
  fi
done

if [ "$viste" -eq 0 ]; then
  echo "nell'APK non ci sono librerie: non e' il programma del tablet" >&2
  exit 2
fi
[ "$guai" -eq 0 ] || exit 1
