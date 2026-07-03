{
  stdenv,
  fetchurl,
  symlinkJoin,
  lib,
  zstd,
  mcfgthreads,
}:

let
  mirror = "https://mirror.nju.edu.cn/msys2/mingw/ucrt64";

  fetchPkg =
    {
      name,
      sha256,
    }:
    stdenv.mkDerivation {
      pname = name;
      version = "unpacked";
      src = fetchurl {
        url = "${mirror}/${name}-any.pkg.tar.zst";
        inherit sha256;
      };
      nativeBuildInputs = [ zstd ];
      unpackPhase = ''
        ${zstd}/bin/zstd -d -c $src | tar xf -
      '';
      installPhase = ''
        mkdir -p $out/ucrt64
        cp -r ucrt64/* $out/ucrt64/ 2>/dev/null || true
        cp -r mingw64/* $out/ucrt64/ 2>/dev/null || true
        for pc in $(find $out -name '*.pc' -type f 2>/dev/null); do
          substituteInPlace "$pc" --replace-quiet "prefix=/ucrt64" "prefix=$out/ucrt64"
        done
        for lib in $(find $out -name '*.a' -type f 2>/dev/null); do
          if [[ "$lib" != *.dll.a ]]; then rm -f "$lib"; fi
        done
      '';
      dontFixup = true;
    };

  packages = [
    (fetchPkg {
      name = "mingw-w64-ucrt-x86_64-adwaita-icon-theme-50.0-1";
      sha256 = "sha256-NXzIFobMeIGNfLkCtiFJqPipTkx0rusapgftIgzlzbE=";
    })
    (fetchPkg {
      name = "mingw-w64-ucrt-x86_64-adwaita-icon-theme-legacy-46.2-1";
      sha256 = "sha256-82XFckp4KeXMUvExzWZPA5H5ls9Fa8faYZbkzFzoKQM=";
    })
    (fetchPkg {
      name = "mingw-w64-ucrt-x86_64-appstream-1.1.2-1";
      sha256 = "sha256-3WSwkQPcCZUZeelVI2zkvS0x1VOx4O83YaO/JZVxqWA=";
    })
    (fetchPkg {
      name = "mingw-w64-ucrt-x86_64-brotli-1.2.0-1";
      sha256 = "sha256-nMiWZUlupQR1FHbq+6FcccVUqvMAS6vCAE98B//WBRQ=";
    })
    (fetchPkg {
      name = "mingw-w64-ucrt-x86_64-bzip2-1.0.8-3";
      sha256 = "sha256-ky2ixjsj5qREh1frNvsZip5REhh0QIJwr+LJG62lE8c=";
    })
    (fetchPkg {
      name = "mingw-w64-ucrt-x86_64-ca-certificates-20250419-1";
      sha256 = "sha256-mrtT1dj9xPT7b/NKNvti4cn1KnFNoaEjT6Drx7V1pfE=";
    })
    (fetchPkg {
      name = "mingw-w64-ucrt-x86_64-cairo-1.18.4-4";
      sha256 = "sha256-BazxGz3LMkZ8pbeaiBqujn2KD7pg3SQxRixGNm1mdQY=";
    })
    (fetchPkg {
      name = "mingw-w64-ucrt-x86_64-c-ares-1.34.6-1";
      sha256 = "sha256-Tg9spagVlZWYzho+46FU7a0wh6WqMYXvwNxysWBffxI=";
    })
    (fetchPkg {
      name = "mingw-w64-ucrt-x86_64-curl-8.21.0-1";
      sha256 = "sha256-7I55OnpPSxiu3uN/1rYOieAwJtegdPvtZxijyHOhXwY=";
    })
    (fetchPkg {
      name = "mingw-w64-ucrt-x86_64-directx-headers-1.619.1-1";
      sha256 = "sha256-JsOXfPl4jwzWoeBJeyhfoJddlNXRwfdAOyT7BojqbEk=";
    })
    (fetchPkg {
      name = "mingw-w64-ucrt-x86_64-directxmath-3.20.b-1";
      sha256 = "sha256-wBSG6WoCSnNm086t9bi4Jj1Qx9egI0Or3UPEcgDNIj4=";
    })
    (fetchPkg {
      name = "mingw-w64-ucrt-x86_64-egl-headers-1.5.r284.3ae2b7c-1";
      sha256 = "sha256-KXQxWQSM8hb3XQ3D6z4KBKB1FdsX2DmAPlSwYPWusLM=";
    })
    (fetchPkg {
      name = "mingw-w64-ucrt-x86_64-expat-2.8.2-1";
      sha256 = "sha256-WO4ONFiUqBAky+jGdmXEfw+cSjuCxm5FdBttud1Vu3U=";
    })
    (fetchPkg {
      name = "mingw-w64-ucrt-x86_64-fontconfig-2.18.1-1";
      sha256 = "sha256-me4KZZMBKgzRGajypz0wflmdCbm6h1fX2eGN7Bo5NHE=";
    })
    (fetchPkg {
      name = "mingw-w64-ucrt-x86_64-freetype-2.14.3-1";
      sha256 = "sha256-9QK9+e0HqpXiI+GroXzoyy2XWqtQZu7Bei19xPuKHCA=";
    })
    (fetchPkg {
      name = "mingw-w64-ucrt-x86_64-fribidi-1.0.16-1";
      sha256 = "sha256-x/4IULaN6oDEMN+MLJuqzzYbJWTdnt6hsPE9baItrAg=";
    })
    (fetchPkg {
      name = "mingw-w64-ucrt-x86_64-gcc-libs-16.1.0-5";
      sha256 = "sha256-TatUyVdW2j4Yyjda5Mf963CfwEvyCesTHBIOJ3BPtbM=";
    })
    (fetchPkg {
      name = "mingw-w64-ucrt-x86_64-gdk-pixbuf2-2.44.7-1";
      sha256 = "sha256-+Sc3RmyR0ljR8p4B1nvCeKh5X+2yw2/3YAPWaYLoRVQ=";
    })
    (fetchPkg {
      name = "mingw-w64-ucrt-x86_64-gettext-runtime-1.0-1";
      sha256 = "sha256-umk92krDda92zkgf86bnSBKGVGzH3G1WxwIdrjQIQVc=";
    })
    (fetchPkg {
      name = "mingw-w64-ucrt-x86_64-giflib-6.1.3-1";
      sha256 = "sha256-nnrI/rDir24PzjyOrRvXvK8BhmHQgDOFjB4mV6BPbxo=";
    })
    (fetchPkg {
      name = "mingw-w64-ucrt-x86_64-gles-headers-3.2.r1065.7fc154c-1";
      sha256 = "sha256-BmAHpQvQjwYrVJTyxIEaIZ4aZfTs3OK+tFP22di5IJg=";
    })
    (fetchPkg {
      name = "mingw-w64-ucrt-x86_64-glib2-2.88.2-1";
      sha256 = "sha256-NnPI3JYhOHQq8jVwm9pTKmXV2spHSfo3H0Mjz/mt6pc=";
    })
    (fetchPkg {
      name = "mingw-w64-ucrt-x86_64-gmp-6.3.0-2";
      sha256 = "sha256-6Cp1lopVZISlAIRXgjioTrYPuT40mG/WaVxTeXW9Oeo=";
    })
    (fetchPkg {
      name = "mingw-w64-ucrt-x86_64-gnutls-3.8.13-2";
      sha256 = "sha256-8cBn4CWsPmAIP8Ul/LjTso1j009UuzyFgKKO9onAAWs=";
    })
    (fetchPkg {
      name = "mingw-w64-ucrt-x86_64-graphene-1.10.8-3";
      sha256 = "sha256-SuSSCKz0nehPKrvpVVwtIvUatSwVCfWsi7XOf8rwfVc=";
    })
    (fetchPkg {
      name = "mingw-w64-ucrt-x86_64-graphite2-1.3.15-1";
      sha256 = "sha256-NMofiL/xDvGCprHI5WItUCnxOwV0jjYa8TtVK0IuqEk=";
    })
    (fetchPkg {
      name = "mingw-w64-ucrt-x86_64-gst-plugins-bad-libs-1.28.4-1";
      sha256 = "sha256-JvZexWXr/sfW8sLze19iL5Ijc1Nsl61/o4N+ZAMU30M=";
    })
    (fetchPkg {
      name = "mingw-w64-ucrt-x86_64-gst-plugins-base-1.28.4-1";
      sha256 = "sha256-e7Vb36VjWBC7/0t+TYFDct+4GMe2vqYNI0Czt07UANA=";
    })
    (fetchPkg {
      name = "mingw-w64-ucrt-x86_64-gstreamer-1.28.4-1";
      sha256 = "sha256-M7Sc+CadD0fOZYVpECVTvl1YCcm4hAqukpwquqg9LXs=";
    })
    (fetchPkg {
      name = "mingw-w64-ucrt-x86_64-gtk4-4.22.4-1";
      sha256 = "sha256-8914upbwH89VKdzgrnoXANTxNOCoTvZknbRBhFBN1co=";
    })
    (fetchPkg {
      name = "mingw-w64-ucrt-x86_64-gtk-update-icon-cache-3.24.52-1";
      sha256 = "sha256-7t/gHSJklmmblbBls+wqT7PFGXmy/WSVQaVg0flJrhg=";
    })
    (fetchPkg {
      name = "mingw-w64-ucrt-x86_64-harfbuzz-14.2.1-1";
      sha256 = "sha256-I1E6MxEWpFluDNcKRvnDAG2btuqcONWU7oISuDwGDJA=";
    })
    (fetchPkg {
      name = "mingw-w64-ucrt-x86_64-hicolor-icon-theme-0.18-1";
      sha256 = "sha256-HvANfANJNaR5+VGjansOF8/cSCdl6GCSQGG5jmlINAw=";
    })
    (fetchPkg {
      name = "mingw-w64-ucrt-x86_64-iso-codes-4.20.1-1";
      sha256 = "sha256-TlEP2k1kvK3315128C9wgOAWvzd1mwpf5bLmVVPkl4U=";
    })
    (fetchPkg {
      name = "mingw-w64-ucrt-x86_64-jbigkit-2.1-5";
      sha256 = "sha256-M04Vav/QBS3orlSexwRNOyzbHwxyVpZZYZ+46CDl2Hg=";
    })
    (fetchPkg {
      name = "mingw-w64-ucrt-x86_64-json-glib-1.10.8-2";
      sha256 = "sha256-OwdEO+ebe+gGLwhdRFMWH8cUQXAElKOTgH92wKvaL8Y=";
    })
    (fetchPkg {
      name = "mingw-w64-ucrt-x86_64-lerc-4.1.0-1";
      sha256 = "sha256-Ej7+YL89K9KI/Q3FnRqddivG553dmsfcznZOGTH497M=";
    })
    (fetchPkg {
      name = "mingw-w64-ucrt-x86_64-libadwaita-1.9.1-1";
      sha256 = "sha256-lYPC78tjHTcaokgPdE7AHXGSmKKLYtGww8XWk+Nxc/w=";
    })
    (fetchPkg {
      name = "mingw-w64-ucrt-x86_64-libb2-0.98.1-3";
      sha256 = "sha256-37VUY1EZ08Rq2wDJ0MWJuzjRyweBsQ7PMd4HzjiL0MA=";
    })
    (fetchPkg {
      name = "mingw-w64-ucrt-x86_64-libdatrie-0.2.14-1";
      sha256 = "sha256-6pJ3eAGtMlJ4GTo2KWdUB1KuNwNQFt8va87i2JkooZU=";
    })
    (fetchPkg {
      name = "mingw-w64-ucrt-x86_64-libdeflate-1.25-1";
      sha256 = "sha256-RxFe3QL2E5oLYuu2pzrsnYk2ZJBfcrmOh69b79t9xp0=";
    })
    (fetchPkg {
      name = "mingw-w64-ucrt-x86_64-libepoxy-1.5.10-7";
      sha256 = "sha256-xsaZTo7EkHHwbMH65HVAa98XYD1Gh60LysLhEHjP18g=";
    })
    (fetchPkg {
      name = "mingw-w64-ucrt-x86_64-libffi-3.6.0-1";
      sha256 = "sha256-w0J75Kg2DX8565X8G+/iFddYzypt1lj/UYUoGBOYMzk=";
    })
    (fetchPkg {
      name = "mingw-w64-ucrt-x86_64-libfyaml-0.9.5-1";
      sha256 = "sha256-pP80F10NwnGmwtBGULRSqBxLWM6z9jNqGKkurhJ4hlg=";
    })
    (fetchPkg {
      name = "mingw-w64-ucrt-x86_64-libiconv-1.19-1";
      sha256 = "sha256-mlAPOMK5GAh0HGL650az6RELM6Hs9cMPoMZtvt3ffhY=";
    })
    (fetchPkg {
      name = "mingw-w64-ucrt-x86_64-libidn2-2.3.8-4";
      sha256 = "sha256-1fS16dmf97KphtHcDHPaK82TumhGK+ex3rvW73Y3H1c=";
    })
    (fetchPkg {
      name = "mingw-w64-ucrt-x86_64-libjpeg-turbo-3.2.0-1";
      sha256 = "sha256-fAAdAN9VfP7paa1D03ucMa4ihp6KrECRqJ3Drm2V4R0=";
    })
    (fetchPkg {
      name = "mingw-w64-ucrt-x86_64-libnice-0.1.23-1";
      sha256 = "sha256-d2n3Y06c/1EK3VmvMpEQ1xF9WEvDm6D/hUwtBjXb4jc=";
    })
    (fetchPkg {
      name = "mingw-w64-ucrt-x86_64-libogg-1.3.6-1";
      sha256 = "sha256-EQo0I56PEi2hXgQbJgHi88bUyQnXOL35eHAGAeD2REk=";
    })
    (fetchPkg {
      name = "mingw-w64-ucrt-x86_64-libpng-1.6.58-1";
      sha256 = "sha256-u/tutiRrAd+Q2C1s/4TwpOmNrQKaSidTiYyW/KrbZsc=";
    })
    (fetchPkg {
      name = "mingw-w64-ucrt-x86_64-libpsl-0.21.5-3";
      sha256 = "sha256-2W4LtQrD+LbkSjnqjejRhvewj2k4sGdutpp7ofEnvPQ=";
    })
    (fetchPkg {
      name = "mingw-w64-ucrt-x86_64-librsvg-2.62.3-1";
      sha256 = "sha256-7gFWxfvgtOwa83rpNnnFqmwjSAAUUFAP2GfqEt/L4bk=";
    })
    (fetchPkg {
      name = "mingw-w64-ucrt-x86_64-libssh2-1.11.1-2";
      sha256 = "sha256-DpExbfpJedyrv6nnFSjsmILs5hnOVJyyUxIIm0dU+mU=";
    })
    (fetchPkg {
      name = "mingw-w64-ucrt-x86_64-libsystre-1.0.2-2";
      sha256 = "sha256-9f44p3w6C8zf0zGRdGcCiUvzY5G/DrVbp6jPC3gWk6c=";
    })
    (fetchPkg {
      name = "mingw-w64-ucrt-x86_64-libtasn1-4.21.0-1";
      sha256 = "sha256-0JiyeDTDslFCtvWqmgylcxqOQzTCMnhkBssUndi94ro=";
    })
    (fetchPkg {
      name = "mingw-w64-ucrt-x86_64-libthai-0.1.30-1";
      sha256 = "sha256-yQ67yj57TDT0s/+lAbqNAM2vY0DP99s8RtedXgHvIHw=";
    })
    (fetchPkg {
      name = "mingw-w64-ucrt-x86_64-libtheora-1.2.0-1";
      sha256 = "sha256-fF18qtx9oLKTzLPrjtnQDzNf1iUnoWz9Kk/dykPY3hE=";
    })
    (fetchPkg {
      name = "mingw-w64-ucrt-x86_64-libtiff-4.7.1-1";
      sha256 = "sha256-zDCnK5V+nKuhFB0mMkkj3WLzcWB964RsmNilLqD+leU=";
    })
    (fetchPkg {
      name = "mingw-w64-ucrt-x86_64-libtre-0.9.0-2";
      sha256 = "sha256-1Q8iVFxQ18DNWER0u6g/DHos2KqxrtGtbB+cZFLzxPw=";
    })
    (fetchPkg {
      name = "mingw-w64-ucrt-x86_64-libunistring-1.4.2-1";
      sha256 = "sha256-54QnPaGbvcyRy7UQteJ8OsAhsCYUBzTu/M5cy9h4vZc=";
    })
    (fetchPkg {
      name = "mingw-w64-ucrt-x86_64-libva-2.23.0-1";
      sha256 = "sha256-D+y6UKzBztwxnRPy5AX6ZsR5273SZXyLOrK/N9jZCkc=";
    })
    (fetchPkg {
      name = "mingw-w64-ucrt-x86_64-libvorbis-1.3.7-2";
      sha256 = "sha256-pYXn2uLcqLLrmz6tBMPzXM5wUT2jYBhlA0QC0Y5UCdw=";
    })
    (fetchPkg {
      name = "mingw-w64-ucrt-x86_64-libwebp-1.6.0-1";
      sha256 = "sha256-NP+DlsQD1ITId3rUmtD8Bt+5T6328UvPhNUMRbxv0X4=";
    })
    (fetchPkg {
      name = "mingw-w64-ucrt-x86_64-libwinpthread-14.0.0.r150.g6b5798fd4-1";
      sha256 = "sha256-Eajnn+9r4+mMywj/mpywQ2/vEyD21Z1oKFEwKQbPBMw=";
    })
    (fetchPkg {
      name = "mingw-w64-ucrt-x86_64-libxml2-2.15.3-1";
      sha256 = "sha256-blLi0/iHCY/y+5jTpMqPyPH9CtDWZD92uaXVw+AwGeE=";
    })
    (fetchPkg {
      name = "mingw-w64-ucrt-x86_64-libxmlb-0.3.25-1";
      sha256 = "sha256-yRxVBVBJLRwkoV8MGvAEkWcDyfmyEiQxSKoxUduuVGg=";
    })
    (fetchPkg {
      name = "mingw-w64-ucrt-x86_64-lzo2-2.10-3";
      sha256 = "sha256-cXyAZZlr6lFUNRig06tGg3v5qZDEGIY0i9L6lITqI/M=";
    })
    (fetchPkg {
      name = "mingw-w64-ucrt-x86_64-nettle-3.10.2-1";
      sha256 = "sha256-Mn+q2kNL81qCAddQ6AbVIy/vYqIUZ8WCLWNA7nvN/i0=";
    })
    (fetchPkg {
      name = "mingw-w64-ucrt-x86_64-nghttp2-1.69.0-1";
      sha256 = "sha256-P7XWjc8O611RsxMbRbD2qWQnNPDyGlEf4WNUVxrHR0M=";
    })
    (fetchPkg {
      name = "mingw-w64-ucrt-x86_64-nghttp3-1.17.0-1";
      sha256 = "sha256-lylrFIGB0O+m4tXqIhc1fppPiH5pFFiq8Wc+wyDA5q4=";
    })
    (fetchPkg {
      name = "mingw-w64-ucrt-x86_64-ngtcp2-1.24.0-1";
      sha256 = "sha256-f2JjYIdImi2Z6Uox8Rvnk4M6rgaqlbu2lB6bl+vMIRE=";
    })
    (fetchPkg {
      name = "mingw-w64-ucrt-x86_64-openssl-3.6.3-1";
      sha256 = "sha256-bKs+NbKwXaOjQSbz/bek0MlChCyM3jVfU6B6cXaEMTo=";
    })
    (fetchPkg {
      name = "mingw-w64-ucrt-x86_64-opus-1.6.1-1";
      sha256 = "sha256-e48Xb5zAQxtSAGhhwKLgg9c9S0enOrdJdVtTef9nMlI=";
    })
    (fetchPkg {
      name = "mingw-w64-ucrt-x86_64-orc-0.4.42-1";
      sha256 = "sha256-pNhQ6CW86l3gRh6MH2ix0WMcPGzlZxc2CX6pcWxV8mU=";
    })
    (fetchPkg {
      name = "mingw-w64-ucrt-x86_64-p11-kit-0.26.2-1";
      sha256 = "sha256-wKNj1rEYpvSxNT5jBo69e0rgOTD6vmMhX/pu40bCWOU=";
    })
    (fetchPkg {
      name = "mingw-w64-ucrt-x86_64-pango-1.58.0-1";
      sha256 = "sha256-gCnyKVOVwWZkycYxrSEbEQXi+y2jOVTFwUVMMMHhZsA=";
    })
    (fetchPkg {
      name = "mingw-w64-ucrt-x86_64-pcre2-10.47-1";
      sha256 = "sha256-g5vEZC+UxE6U4zHJCSxtGGse3FTf32qByyBi9jhBcCM=";
    })
    (fetchPkg {
      name = "mingw-w64-ucrt-x86_64-pixman-0.46.4-3";
      sha256 = "sha256-TAG5hlY9K1961vzsLQ7JkV0vJC3axerN3BmXGWQkHMU=";
    })
    (fetchPkg {
      name = "mingw-w64-ucrt-x86_64-python-3.14.6-1";
      sha256 = "sha256-pCz8KdP0FiNmvhFN3+rBHtpYd1JIszAfgMWtuQXvWBo=";
    })
    (fetchPkg {
      name = "mingw-w64-ucrt-x86_64-python-packaging-26.2-1";
      sha256 = "sha256-InGea0nr8K8KzmkhP80m9EJtkcNcLcT+qeSIht5lYao=";
    })
    (fetchPkg {
      name = "mingw-w64-ucrt-x86_64-shared-mime-info-2.5.1-1";
      sha256 = "sha256-cdoeQDI3T4niyfz9j2JrW2Tm01nlMjXNllNGXojUNdY=";
    })
    (fetchPkg {
      name = "mingw-w64-ucrt-x86_64-sqlite3-3.53.3-1";
      sha256 = "sha256-rzLCb3cf3gtua1b8/JxhSiPeQmP7lCFcWHk7fcw3DOU=";
    })
    (fetchPkg {
      name = "mingw-w64-ucrt-x86_64-tcl-8.6.18-1";
      sha256 = "sha256-DDpQ6lvoxRkOWKH9xGwJ+BE60bhVdFh1ZGt6eKAiFd8=";
    })
    (fetchPkg {
      name = "mingw-w64-ucrt-x86_64-tk-8.6.18-1";
      sha256 = "sha256-v6ARiGAyligz3ck66bUIeY7FZiW5CTzG0P9clVB8ecY=";
    })
    (fetchPkg {
      name = "mingw-w64-ucrt-x86_64-tzdata-2026b-1";
      sha256 = "sha256-+t5gxo3G2+Sv3ckRHuI1oK6Q/zmKt8wSIr+weRocjOQ=";
    })
    (fetchPkg {
      name = "mingw-w64-ucrt-x86_64-vulkan-loader-1~1.4.350.1-1";
      sha256 = "sha256-SPKyhXxVPPVk/bDrpwmuAIec1xkxleDxfCnKm+tCSGM=";
    })
    (fetchPkg {
      name = "mingw-w64-ucrt-x86_64-wineditline-2.208-1";
      sha256 = "sha256-sHs03hZp4Wz9Z0NpGRT1pmrUATUAu/Dkx6dkQEauexw=";
    })
    (fetchPkg {
      name = "mingw-w64-ucrt-x86_64-xz-5.8.3-1";
      sha256 = "sha256-hArwGS9LZ74JI454MknHjMFnKYHn0KzFxnpr0LS8oMs=";
    })
    (fetchPkg {
      name = "mingw-w64-ucrt-x86_64-zlib-1.3.2-2";
      sha256 = "sha256-hBQBGCl20vnhflwOuqxR8qgBQUDqU9Z2Jekcj7PIXqA=";
    })
    (fetchPkg {
      name = "mingw-w64-ucrt-x86_64-zstd-1.5.7-2";
      sha256 = "sha256-29uEJygARqK0Fpd4CqTFKYO3CAgrDaR1WVHcO+qWyok=";
    })
  ];

in
let
  merged = symlinkJoin {
    name = "msys2-windows-deps";
    paths = packages;
  };
in
stdenv.mkDerivation {
  name = "msys2-windows-deps";
  src = merged;
  dontUnpack = true;
  buildPhase = ''
    cp -r $src $out
    chmod -R u+w $out
    ln -sf ${mcfgthreads}/lib/libmcfgthread.a $out/ucrt64/lib/libpthread.a
  '';
  dontFixup = true;
}
