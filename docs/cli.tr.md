# Komut satırı başvurusu

[English](cli.md) · [README'ye dön](../README.tr.md)

Her komutun kendi yardım metni vardır: `leafcut --help`, `leafcut split --help` gibi. Yardım
metinleri şimdilik yalnızca İngilizce.

| Komut      | Ne yapar |
|------------|----------|
| `merge`    | Birden fazla PDF'i verilen sırayla tek dosyada birleştirir |
| `split`    | Her sayfayı, her aralığı (`1-3,7,10-`) ya da her N sayfayı ayrı dosyaya böler |
| `extract`  | Seçilen sayfaları yeni bir dosyaya yazar (istediğiniz sırada, tekrar edebilir) |
| `delete`   | Sayfaları siler |
| `rotate`   | Tüm sayfaları ya da seçilenleri saat yönünde 90/180/270 derece döndürür |
| `reorder`  | Sayfaları yeni bir sıraya koyar ya da tersine çevirir |
| `compress` | Kayıpsız küçültme (bkz. [Sınırlamalar](../README.tr.md#sınırlamalar)) |
| `info`     | Sayfa sayısı, sayfa boyutları, PDF sürümü ve üst veri (metin ya da JSON) |
| `meta`     | Başlık, yazar, konu, anahtar kelimeler, oluşturan ve üreten uygulama bilgisini ayarlar ya da siler |

## Örnekler

```sh
# Birleştirme (çıktı dosyası açıkça belirtilmelidir)
leafcut merge kapak.pdf rapor.pdf ekler.pdf -o tam.pdf

# Tek tek sayfalara bölme: kitap-01.pdf, kitap-02.pdf, ...
leafcut split kitap.pdf

# Aralıklara göre bölme: kitap-01-03.pdf, kitap-07.pdf, kitap-10-12.pdf
leafcut split kitap.pdf --ranges 1-3,7,10- --out-dir parcalar/

# Her dosyada on sayfa
leafcut split kitap.pdf --every 10

# 1. ve 4-6. sayfaları al (varsayılan çıktı: sozlesme-extract.pdf)
leafcut extract sozlesme.pdf --pages 1,4-6

# İlk iki sayfayı ve 9. sayfadan sonrasını sil
leafcut delete tarama.pdf --pages -2,9- -o tarama-kirpilmis.pdf

# Tüm sayfaları ya da yalnızca bazılarını döndür
leafcut rotate tarama.pdf --angle 90
leafcut rotate tarama.pdf --angle 180 --pages 2,4-5 -o duzeltilmis.pdf

# 3. sayfayı başa al ya da belgeyi tersine çevir
leafcut reorder sunum.pdf --order 3,1,2,4-
leafcut reorder sunum.pdf --reverse

# Küçültme
leafcut compress rapor.pdf -o rapor-kucuk.pdf

# İnceleme
leafcut info rapor.pdf
leafcut info rapor.pdf --pages        # her sayfanın boyutunu listeler
leafcut info *.pdf --json

# Üst veri (boş değer o alanı siler)
leafcut meta tez.pdf --title "Yüksek Lisans Tezi" --author "Ayşe Yılmaz"
leafcut meta tez.pdf --clear -o tez-temiz.pdf
```

## Sayfa aralıkları

Sayfa numaraları 1'den başlar ve virgülle ayrılır: `5`, `1-3`, `10-` (sona kadar), `-3` (baştan
3. sayfaya kadar), `all` (tümü). Örnek: `1-3,7,10-`.

## Çıktı dosyaları

Tek dosya üreten komutlar, `-o DOSYA` vermediğiniz sürece girdinin yanına `GIRDI-<işlem>.pdf`
adıyla yazar. leafcut şunları yapmayı reddeder:

- girdi dosyasının üzerine yazmak (gerçekten istiyorsanız `--in-place` verin),
- var olan bir çıktının üzerine yazmak (`--force` verin).

Çıktılar önce hedef klasörde geçici bir dosyaya yazılır, sonra yeniden adlandırılır. Yarıda
kesilen bir işlem yarım kalmış bir PDF bırakmaz.

## Çıkış kodları

| Kod | Anlamı |
|-----|--------|
| `0` | başarılı |
| `1` | hata: okunamayan dosya, bozuk PDF, G/Ç |
| `2` | hatalı kullanım: yanlış argüman, aralık dışı sayfa |
| `3` | şifreli PDF (desteklenmiyor) |
| `4` | çıktı dosyası zaten var |
