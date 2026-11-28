#!/system/bin/sh
# Fa di un tablet Samsung un dispositivo che e' solo NOVA (D452).
#
# Si lancia SUL tablet, dal cavo, dopo un ripristino di fabbrica senza
# account e con il programma di NOVA gia' installato:
#
#   adb push core/tablet/prepara.sh /data/local/tmp/prepara.sh
#   adb shell sh /data/local/tmp/prepara.sh            # fa
#   adb shell sh /data/local/tmp/prepara.sh indietro   # disfa
#
# Cosa fa:
# - spegne i programmi che su un tablet «solo NOVA» non servono (Google,
#   le app di Samsung, i servizi che lavorano in sottofondo). Li SPEGNE, non
#   li cancella: stanno nella parte del sistema che non si scrive, e
#   «indietro» li riaccende tutti;
# - toglie il blocco dello schermo;
# - mette NOVA come schermata di casa, e per ultima spegne quella di Samsung:
#   da li' in poi all'accensione, e col tasto Home, c'e' NOVA.
#
# Cosa NON tocca, di proposito: il WebView (la finestra di NOVA e' sua), la
# tastiera, le Impostazioni, l'interfaccia di sistema, il Wi-Fi, il
# Bluetooth, e tutto Knox: spegnere un pezzo di Knox puo' lasciare il
# tablet a girare in tondo all'avvio.
#
# L'elenco e' quello del Galaxy Tab A8 (SM-X200, Android 14, One UI 6.1)
# com'era il 28 novembre 2026 dopo il ripristino. Un pacchetto che su un
# altro tablet non c'e' si salta.

NOVA="io.nova.tablet"
CASA_DI_SAMSUNG="com.sec.android.app.launcher"

# Un nome per riga; quello che segue il cancelletto e' un commento.
DA_SPEGNERE="
# --- Google: il negozio, i servizi, le app ---
com.android.vending
com.google.android.gms
com.google.android.gsf
com.google.android.gms.location.history
com.google.android.gms.supervision
com.google.android.googlequicksearchbox
com.google.android.apps.maps
com.google.android.apps.messaging
com.google.android.apps.tachyon
com.google.android.apps.turbo
com.google.android.apps.restore
com.google.android.apps.photos
com.google.android.apps.docs
com.google.android.apps.youtube.music
com.google.android.videos
com.google.android.youtube
com.google.android.gm
com.google.ar.core
com.google.android.as
com.google.android.as.oss
com.google.android.tts
com.google.android.feedback
com.google.android.partnersetup
com.google.android.syncadapters.calendar
com.google.android.printservice.recommendation
com.google.android.apps.carrier.carrierwifi
com.google.android.apps.setupwizard.searchselector
com.android.chrome
com.sec.android.app.chromecustomizations
# --- messi da altri ---
com.microsoft.skydrive
com.hiya.star
de.axelspringer.yana.zeropage
com.aura.oobe.samsung.gl
# --- il negozio e l'account di Samsung ---
com.sec.android.app.samsungapps
com.sec.android.app.billing
com.osp.app.signin
com.samsung.android.scloud
com.samsung.android.mobileservice
com.samsung.android.fmm
com.sec.spp.push
# --- le app di Samsung ---
com.samsung.android.app.spage
com.samsung.android.game.gamehome
com.samsung.android.calendar
com.samsung.android.app.contacts
com.samsung.android.dialer
com.samsung.android.messaging
com.samsung.android.video
com.sec.android.gallery3d
com.sec.android.app.myfiles
com.sec.android.app.clockpackage
com.samsung.android.app.clockpack
com.sec.android.daemonapp
com.samsung.android.kidsinstaller
com.samsung.android.app.dressroom
com.samsung.android.stickercenter
com.samsung.android.forest
com.sec.android.widgetapp.webmanual
com.sec.android.mimage.photoretouching
com.sec.android.autodoodle.service
com.samsung.app.newtrim
com.sec.android.app.ve.vebgm
com.samsung.android.widget.pictureframe
com.samsung.storyservice
# --- Bixby, le abitudini, i suggerimenti ---
com.samsung.android.app.routines
com.samsung.android.app.settings.bixby
com.samsung.android.intellivoiceservice
com.samsung.android.rubin.app
# --- condividere, proiettare, passare da un dispositivo all'altro ---
com.samsung.android.app.sharelive
com.samsung.android.allshare.service.mediashare
com.samsung.android.smartmirroring
com.samsung.android.galaxycontinuity
com.samsung.android.mdx.kit
com.samsung.android.mdx.quickboard
com.samsung.android.mdecservice
com.samsung.android.smartswitchassistant
com.sec.android.easyMover
com.sec.android.easyMover.Agent
com.samsung.android.easysetup
com.samsung.android.beaconmanager
com.samsung.android.aware.service
com.samsung.android.mapsagent
# --- i pannelli sul bordo, gli attrezzi per le schermate ---
com.samsung.android.app.appsedge
com.samsung.android.app.clipboardedge
com.samsung.android.app.taskedge
com.samsung.android.app.cocktailbarservice
com.samsung.android.service.peoplestripe
com.samsung.android.app.smartcapture
# --- la voce di Samsung: NOVA parla con la sua ---
com.samsung.SMT
com.samsung.SMT.lang_de_de_f00
com.samsung.SMT.lang_en_gb_f00
com.samsung.SMT.lang_en_us_l03
com.samsung.SMT.lang_es_es_f00
com.samsung.SMT.lang_es_mx_f00
com.samsung.SMT.lang_fr_fr_f00
com.samsung.SMT.lang_it_it_f00
com.samsung.SMT.lang_pt_br_f00
com.samsung.SMT.lang_ru_ru_f00
com.samsung.android.accessibility.talkback
# --- chi raccoglie dati e chi cerca aggiornamenti ---
com.sec.android.diagmonagent
com.samsung.android.dqagent
com.samsung.android.knox.analytics.uploader
com.samsung.android.bbc.bbcagent
com.samsung.android.app.updatecenter
com.sec.android.soagent
com.wssyncmldm
# --- pezzi di Android che qui non servono ---
com.android.bookmarkprovider
com.android.providers.partnerbookmarks
com.android.dreams.basic
com.android.dreams.phototable
com.android.egg
com.android.wallpaper.livepicker
com.android.printspooler
com.android.bips
"

elenco() {
  echo "$DA_SPEGNERE" | sed 's/#.*//' | tr -d ' \t' | grep -v '^$'
}

# Gli elenchi si chiedono una volta sola: una chiamata a `pm` per ogni
# pacchetto farebbe durare lo script dei minuti.
PRESENTI=$(pm list packages 2>/dev/null)
NEL_SISTEMA=$(pm list packages -u 2>/dev/null)
SPENTI=$(pm list packages -d 2>/dev/null)

fra() {
  echo "$1" | grep -qx "package:$2"
}

if [ "$1" = "indietro" ]; then
  pm enable "$CASA_DI_SAMSUNG" >/dev/null 2>&1
  cmd package set-home-activity "$CASA_DI_SAMSUNG/.activities.LauncherActivity" >/dev/null 2>&1
  riaccesi=0
  for p in $(elenco); do
    cmd package install-existing --user 0 "$p" >/dev/null 2>&1
    pm enable "$p" >/dev/null 2>&1 && riaccesi=$((riaccesi + 1))
  done
  echo "riaccesi: $riaccesi; la schermata di casa e' di nuovo quella di Samsung"
  exit 0
fi

if ! fra "$PRESENTI" "$NOVA"; then
  echo "NOVA ($NOVA) non e' installata: prima si installa, poi si prepara il tablet."
  exit 1
fi

spenti=0; gia=0; assenti=0; rifiutati=""
for p in $(elenco); do
  if ! fra "$NEL_SISTEMA" "$p"; then
    assenti=$((assenti + 1)); continue
  fi
  if fra "$SPENTI" "$p" || ! fra "$PRESENTI" "$p"; then
    gia=$((gia + 1)); continue
  fi
  if pm disable-user --user 0 "$p" >/dev/null 2>&1; then
    spenti=$((spenti + 1))
  # Alcuni il sistema non li lascia spegnere ma li lascia togliere a questo
  # utente: restano nel sistema, e «indietro» li rimette.
  elif pm uninstall -k --user 0 "$p" >/dev/null 2>&1; then
    spenti=$((spenti + 1))
  else
    rifiutati="$rifiutati $p"
  fi
done
echo "spenti: $spenti; gia' spenti: $gia; non presenti: $assenti"
[ -n "$rifiutati" ] && echo "il sistema non ha lasciato spegnere:$rifiutati"

# Niente blocco dello schermo: all'accensione si arriva dritti a NOVA.
locksettings set-disabled true >/dev/null 2>&1
# A batteria lo schermo si spegne dopo dieci minuti; sotto carica lo tiene
# acceso NOVA.
settings put system screen_off_timeout 600000
settings put global stay_on_while_plugged_in 0

# NOVA e' la schermata di casa; quella di Samsung si spegne per ultima, e
# solo se NOVA ha preso il suo posto.
cmd package set-home-activity "$NOVA/.MainActivity" >/dev/null 2>&1
casa=$(cmd package resolve-activity --brief -c android.intent.category.HOME -a android.intent.action.MAIN | tail -1)
if [ "$casa" = "$NOVA/.MainActivity" ]; then
  pm disable-user --user 0 "$CASA_DI_SAMSUNG" >/dev/null 2>&1
  echo "la schermata di casa e' NOVA"
else
  echo "la schermata di casa NON e' NOVA ($casa): quella di Samsung resta accesa"
  exit 1
fi
