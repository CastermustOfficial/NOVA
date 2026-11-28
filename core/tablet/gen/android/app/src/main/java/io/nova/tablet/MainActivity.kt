package io.nova.tablet

import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import android.content.IntentFilter
import android.os.BatteryManager
import android.os.Bundle
import android.view.WindowManager
import androidx.activity.enableEdgeToEdge
import androidx.core.view.ViewCompat
import androidx.core.view.WindowCompat
import androidx.core.view.WindowInsetsCompat
import androidx.core.view.WindowInsetsControllerCompat

/**
 * La finestra di NOVA su un dispositivo che e' solo NOVA (D452).
 *
 * - A tutto schermo: le barre del sistema si nascondono, e tornano per un
 *   attimo strisciando dal bordo.
 * - Lo schermo resta acceso finche' il dispositivo e' sotto carica, come una
 *   cosa appoggiata sulla sua base. A batteria no: si spegne coi tempi del
 *   sistema, se no una batteria vecchia dura un paio d'ore.
 * - Con la tastiera aperta la pagina si stringe nello spazio che resta,
 *   invece di scorrere via: mentre si scrive si vede ancora l'orb.
 */
class MainActivity : TauriActivity() {
  private val corrente = object : BroadcastReceiver() {
    override fun onReceive(contesto: Context?, intento: Intent?) {
      tieniAcceso(intento?.action == Intent.ACTION_POWER_CONNECTED)
    }
  }

  override fun onCreate(savedInstanceState: Bundle?) {
    enableEdgeToEdge()
    super.onCreate(savedInstanceState)
    nascondiLeBarre()
    faiPostoAllaTastiera()
    tieniAcceso(sottoCarica())
    val quando = IntentFilter().apply {
      addAction(Intent.ACTION_POWER_CONNECTED)
      addAction(Intent.ACTION_POWER_DISCONNECTED)
    }
    registerReceiver(corrente, quando)
  }

  override fun onDestroy() {
    unregisterReceiver(corrente)
    super.onDestroy()
  }

  override fun onWindowFocusChanged(hasFocus: Boolean) {
    super.onWindowFocusChanged(hasFocus)
    if (hasFocus) nascondiLeBarre()
  }

  /** Il dispositivo e' attaccato alla corrente? */
  private fun sottoCarica(): Boolean {
    val stato = registerReceiver(null, IntentFilter(Intent.ACTION_BATTERY_CHANGED))
    return (stato?.getIntExtra(BatteryManager.EXTRA_PLUGGED, 0) ?: 0) != 0
  }

  private fun tieniAcceso(si: Boolean) {
    if (si) window.addFlags(WindowManager.LayoutParams.FLAG_KEEP_SCREEN_ON)
    else window.clearFlags(WindowManager.LayoutParams.FLAG_KEEP_SCREEN_ON)
  }

  /** La finestra va da bordo a bordo, e il sistema non la stringe da solo:
   *  il posto per la tastiera si fa qui, col margine in fondo. */
  private fun faiPostoAllaTastiera() {
    ViewCompat.setOnApplyWindowInsetsListener(findViewById(android.R.id.content)) { vista, margini ->
      vista.setPadding(0, 0, 0, margini.getInsets(WindowInsetsCompat.Type.ime()).bottom)
      margini
    }
  }

  private fun nascondiLeBarre() {
    val barre = WindowCompat.getInsetsController(window, window.decorView)
    barre.systemBarsBehavior = WindowInsetsControllerCompat.BEHAVIOR_SHOW_TRANSIENT_BARS_BY_SWIPE
    barre.hide(WindowInsetsCompat.Type.systemBars())
  }
}
