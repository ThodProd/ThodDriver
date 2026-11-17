const $ = selector => document.querySelector(selector);
let drivers = [];
let jobTimer = null;

function setNotice(message = '', error = false) { const notice = $('#notice'); notice.hidden = !message; notice.textContent = message; notice.className = error ? 'notice error' : 'notice'; }
function setBusy(busy, label) { $('#scan').disabled = busy; $('#install').disabled = busy || !drivers.some(d => d.selected); $('#install-folder').disabled = busy; if (label) $('#summary').textContent = label; }
function selected() { return drivers.filter(driver => driver.selected); }
function updateSelected() { const count = selected().length; $('#selected').textContent = count ? `Выбрано обновлений: ${count}` : 'Не выбрано обновлений'; $('#install').disabled = count === 0; }
function render() {
  const root = $('#updates'); root.replaceChildren();
  if (!drivers.length) { root.innerHTML = '<div class="empty">Подходящих обновлений драйверов не найдено.</div>'; updateSelected(); return; }
  for (const driver of drivers) {
    const row = document.createElement('label'); row.className = 'row';
    const box = document.createElement('input'); box.type = 'checkbox'; box.checked = driver.selected; box.addEventListener('change', () => { driver.selected = box.checked; updateSelected(); });
    const info = document.createElement('div'); const title = document.createElement('div'); title.className = 'name'; title.textContent = driver.title; const meta = document.createElement('div'); meta.className = 'meta'; meta.textContent = [driver.class, driver.vendor, driver.date].filter(Boolean).join(' · '); info.append(title, meta);
    const vendor = document.createElement('span'); vendor.className = 'meta'; vendor.textContent = driver.vendor || 'Производитель не указан';
    const date = document.createElement('span'); date.className = 'meta'; date.textContent = driver.date || '—'; row.append(box, info, vendor, date); root.append(row);
  }
  updateSelected();
}
async function scan() {
  setNotice(); setBusy(true, 'Windows Update ищет доступные драйверы…'); $('#updates').innerHTML = '<div class="empty">Поиск может занять несколько минут.</div>';
  try { drivers = await thodHub.call('worker.scan'); drivers.forEach(driver => driver.selected = true); render(); $('#summary').textContent = drivers.length ? `Найдено обновлений драйверов: ${drivers.length}.` : 'Все доступные драйверы уже установлены.'; }
  catch (error) { $('#updates').innerHTML = '<div class="empty">Список обновлений не получен.</div>'; setNotice(`Не удалось проверить обновления: ${error.message}`, true); $('#summary').textContent = 'Проверка не выполнена.'; }
  finally { setBusy(false); updateSelected(); }
}
function showJob(job) { const log = $('#log'); log.open = true; log.querySelector('pre').textContent = job.logs.join('\n'); }
async function trackJob(jobId) {
  clearInterval(jobTimer); setBusy(true, 'Выполняется операция с драйверами…');
  const poll = async () => { try { const job = await thodHub.call('worker.job.status', { jobId }); showJob(job); if (job.state === 'completed' || job.state === 'failed') { clearInterval(jobTimer); setBusy(false, job.state === 'completed' ? (job.rebootRequired ? 'Готово. Перезагрузите Windows.' : 'Готово.') : 'Операция завершилась с ошибкой.'); setNotice(job.error || (job.rebootRequired ? 'Для применения изменений требуется перезагрузка Windows.' : 'Операция завершена.'), job.state === 'failed'); } } catch (error) { clearInterval(jobTimer); setBusy(false, 'Не удалось получить состояние операции.'); setNotice(error.message, true); } };
  await poll(); jobTimer = setInterval(poll, 1200);
}
async function install() { try { const result = await thodHub.call('worker.job.start', { operation:'install', driverIds:selected().map(driver => driver.id), createBackup:$('#backup').checked, createRestorePoint:$('#restore').checked }); await trackJob(result.jobId); } catch (error) { setNotice(error.message, true); } }
async function installFolder() { try { const folder = await thodHub.call('dialog.folder'); if (!folder) return; const result = await thodHub.call('worker.job.start', { operation:'installFolder', folder, createBackup:false, createRestorePoint:$('#restore').checked }); await trackJob(result.jobId); } catch (error) { setNotice(error.message, true); } }
$('#scan').addEventListener('click', scan); $('#install').addEventListener('click', install); $('#install-folder').addEventListener('click', installFolder); $('#all').addEventListener('click', () => { drivers.forEach(driver => driver.selected = true); render(); }); $('#none').addEventListener('click', () => { drivers.forEach(driver => driver.selected = false); render(); });
